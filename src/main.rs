use std::fs;
use std::collections::HashMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use sha2::{Sha256, Digest};
use md5::compute as md5_compute;
use walkdir::WalkDir;
use rayon::prelude::*;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "RustyArchiver", version = "1.0", author = "Your Name <your.email@example.com>", about = "Archives folders with checksum verification")]
struct Cli {
    #[arg(short, long)]
    folder_to_archive: String,
    
    #[arg(short, long)]
    temp_dir: String,
    
    #[arg(short, long, default_value_t = false)]
    move_to_archive: bool,
    
    #[arg(short, long, default_value_t = 4)]
    cores: usize,
    
    #[arg(short, long)]
    archive_dir: Option<String>,
}

// Function to calculate SHA256 checksum of a file
fn calculate_sha256(file_path: &Path) -> io::Result<String> {
    let mut file = fs::File::open(file_path)?;
    let mut sha256 = Sha256::new();
    io::copy(&mut file, &mut sha256)?;
    let result = sha256.finalize();
    Ok(format!("{:x}", result))
}

// Function to calculate MD5 checksum of a file
fn calculate_md5(file_path: &Path) -> io::Result<String> {
    let mut file = fs::File::open(file_path)?;
    let mut buffer = Vec::new();
    io::copy(&mut file, &mut buffer)?;
    let result = md5_compute(&buffer);
    Ok(format!("{:x}", result))
}

// Function to generate a list of checksums for each file in the folder
fn generate_list_of_checksum(folder: &Path, checksum_file: &Path) -> io::Result<()> {
    let mut file = fs::File::create(checksum_file)?;

    // Collect all file paths
    let entries: Vec<_> = WalkDir::new(folder)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .collect();

    // Parallel computation of SHA256 checksums
    let results: Vec<_> = entries.par_iter().map(|entry| {
        let checksum = calculate_sha256(entry.path());
        (checksum, entry.path().display().to_string())
    }).collect();

    // Write the checksums to the file
    for (checksum, path) in results {
        if let Ok(checksum) = checksum {
            writeln!(file, "{} {}", checksum, path)?;
        }
    }

    Ok(())
}

// Function to compress the folder into a tgz file in the temp directory
fn compress_folder(folder: &Path, temp_dir: &Path) -> io::Result<PathBuf> {
    let folder_name = folder.file_name().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "The folder to archive has no valid name"))?;
    let output_file = temp_dir.join(format!("{}.tgz", folder_name.to_str().unwrap()));

    let parent_dir = folder.parent().unwrap_or_else(|| Path::new("."));
    if parent_dir.to_str().unwrap().is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "The folder to archive has no valid parent directory"));
    }

    let status = Command::new("tar")
        .arg("-czf")
        .arg(&output_file)
        .arg("-C")
        .arg(parent_dir)
        .arg(folder_name)
        .status()?;

    if !status.success() {
        return Err(io::Error::new(io::ErrorKind::Other, "Failed to compress folder"));
    }
    Ok(output_file)
}

// Function to copy the compressed file to the archive directory
fn copy_file_to_archive(compressed_file: &Path, archive_dir: &Path) -> io::Result<()> {
    let destination = archive_dir.join(compressed_file.file_name().unwrap());
    fs::copy(compressed_file, &destination)?;
    Ok(())
}

// Function to verify the integrity of the compressed folder by checking checksums
fn verify_compressed_folder(compressed_file: &Path, original_checksum_file: &Path, decompressed_checksum_file: &Path, temp_dir: &Path) -> io::Result<()> {
    let decompressed_dir = temp_dir.join("temp_verification");

    fs::create_dir_all(&decompressed_dir)?;

    let status = Command::new("tar")
        .arg("-xzf")
        .arg(compressed_file)
        .arg("-C")
        .arg(&decompressed_dir)
        .status()?;
    if !status.success() {
        return Err(io::Error::new(io::ErrorKind::Other, "Failed to extract compressed folder"));
    }

    generate_list_of_checksum(&decompressed_dir, decompressed_checksum_file)?;


    // Read and parse the checksum files
    let original_checksums = fs::read_to_string(original_checksum_file)?;
    let decompressed_checksums = fs::read_to_string(decompressed_checksum_file)?;

    let original_map = parse_checksums(&original_checksums)?;
    let decompressed_map = parse_checksums(&decompressed_checksums)?;

    fs::remove_dir_all(&decompressed_dir)?;

    if original_map == decompressed_map {
        println!("Verification of compressed folder successful.");
        Ok(())
    } else {
        Err(io::Error::new(io::ErrorKind::Other, "Checksum mismatch"))
    }
}

// Function to verify the integrity of the copied compressed file by checking MD5 checksums
fn verify_copy_to_archive(original_file: &Path, copied_file: &Path) -> io::Result<()> {
    let original_md5 = calculate_md5(original_file)?;
    let copied_md5 = calculate_md5(copied_file)?;
    if original_md5 != copied_md5 {
        return Err(io::Error::new(io::ErrorKind::Other, "MD5 mismatch between original and copied file"));
    }
    println!("Verification of copied file successful.");
    Ok(())
}

// Function to clean up temporary directories
fn clean_up(temp_dir: &Path) -> io::Result<()> {
    fs::remove_dir_all(temp_dir)?;
    Ok(())
}

// Function to rename the compressed folder based on its MD5 checksum
fn rename_folder(compressed_file: &Path) -> io::Result<PathBuf> {
    let md5_checksum = calculate_md5(compressed_file)?;
    let new_name = format!("{}_{}", md5_checksum, compressed_file.file_name().unwrap().to_str().unwrap());
    let new_path = compressed_file.with_file_name(new_name);
    fs::rename(compressed_file, &new_path)?;
    Ok(new_path)
}

// Function to prepare for archiving by generating checksums and including folder name
fn prepare_archiving(folder_to_archive: &Path, checksum_file: &Path) -> io::Result<()> {
    generate_list_of_checksum(folder_to_archive, checksum_file)
}

// Helper function to parse the checksum file into a HashMap
fn parse_checksums(content: &str) -> io::Result<HashMap<String, String>> {
    let mut map = HashMap::new();
    for line in content.lines() {
        if let Some((checksum, _path)) = line.split_once(' ') {
            map.insert(checksum.to_string(), checksum.to_string());
        }
    }
    Ok(map)
}

fn main() -> io::Result<()> {
    let args = Cli::parse();

    let folder_to_archive = Path::new(&args.folder_to_archive);
    let temp_dir = Path::new(&args.temp_dir);
    let move_to_archive = args.move_to_archive;
    let cores = args.cores;

    // Optional archive directory
    let archive_dir = args.archive_dir.map(|dir| Path::new(&dir).to_path_buf());

    // Initialize the rayon thread pool with the specified number of cores
    rayon::ThreadPoolBuilder::new().num_threads(cores).build_global().unwrap();

    // Define paths for the compressed file and checksum files
    let folder_name = folder_to_archive.file_name().unwrap().to_str().unwrap();
    let original_checksum_file_path = temp_dir.join(format!("{}_checksum.txt", folder_name));
    let decompressed_checksum_file_path = temp_dir.join(format!("{}_checksum_decompressed.txt", folder_name));

    prepare_archiving(folder_to_archive, &original_checksum_file_path)?;
    let compressed_file_path = compress_folder(folder_to_archive, temp_dir)?;
    verify_compressed_folder(&compressed_file_path, &original_checksum_file_path, &decompressed_checksum_file_path, temp_dir)?;
    let renamed_compressed_file_path = rename_folder(&compressed_file_path)?;

    if move_to_archive {
        if let Some(archive_dir) = &archive_dir {
            copy_file_to_archive(&renamed_compressed_file_path, archive_dir)?;
            verify_copy_to_archive(&renamed_compressed_file_path, &archive_dir.join(renamed_compressed_file_path.file_name().unwrap()))?;
        } else {
            eprintln!("Archive directory not specified.");
            std::process::exit(1);
        }
    }

    //clean_up(temp_dir)?;

    println!("Archiving completed successfully.");
    Ok(())
}

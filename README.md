# RustyArchiver

RustyArchiver is a powerful and efficient archiving tool written in Rust. It allows you to compress and decompress files with ease, providing fast performance and a user-friendly command-line interface.

![RustyArchiver Logo](./rustyarchiver.webp)

## Features

- Fast compression 
- Easy-to-use command-line interface
- Cross-platform compatibility

## Installation

To install RustyArchiver, follow these steps:

1. Clone the repository:
    ```bash
    git clone https://github.com/elena-krismer/rustyarchiver.git
    ```

2. Navigate to the project directory:
    ```bash
    cd rustyarchiver
    ```

3. Build the project using Cargo:
    ```bash
    cargo build --release
    ```

4. The compiled binary will be available in the `target/release` directory.

## Usage

RustyArchiver is simple to use via the command line. Below are some examples of common tasks:

```bash
nohup cargo run -- --folder-to-archive ../../Astral/ --temp-dir ../astral_2403/  --cores 8
```

use std::fs::OpenOptions;
use std::path::PathBuf;
use std::{io::BufReader, io::Result};

use clap::Parser;

use squashfs::image::Image;

#[derive(Parser, Debug)]
#[command(version, about, long_about)]
struct Args {
    #[arg(short, long)]
    archive_name: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let sqfs_file = OpenOptions::new().read(true).open(args.archive_name)?;
    let sqfs_file = BufReader::new(sqfs_file);

    let image = Image::new(sqfs_file)?;
    eprintln!("{}", image.superblock());
    let (_, list) = image.inodes()?;

    for item in list {
        eprintln!("{}", item);
    }

    for fragment in image.fragments()? {
        eprintln!("{:?}", fragment);
    }
    Ok(())
}

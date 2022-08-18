use std::{env, fs, io::BufReader, io::Result};

use squashfs::image::Image;

fn main() -> Result<()> {
    let mut args = env::args();
    let mut fname = args.nth(1);
    let fname = fname.get_or_insert("./test.sqfs".into());
    let f = BufReader::new(fs::File::open(fname)?);

    let image = Image::new(f)?;
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

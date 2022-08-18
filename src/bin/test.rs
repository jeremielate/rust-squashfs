use squashfs::{image::Image, inode::InodeHeader};
use std::{
    env::{self},
    fs,
    io::{BufReader, Result},
};

fn main() -> Result<()> {
    let mut args = env::args();
    let mut fname = args.nth(1);
    let fname = fname.get_or_insert("./test.sqfs".into());
    let f = BufReader::new(fs::File::open(fname)?);

    let mut image = Image::new(f)?;

    let (fragments, id_table, lookup_table, _root_inode, inode_headers) = image.read_fs()?;

    let mut directories = 0;
    let mut regulars = 0;
    let mut symlinks = 0;
    let mut devs = 0;
    let mut ipcs = 0;
    for inode_header in &inode_headers {
        match inode_header {
            InodeHeader::Directory(_) | InodeHeader::LDirectory(_) => directories += 1,
            InodeHeader::Regular(_) | InodeHeader::LRegular(_) => regulars += 1,
            InodeHeader::Symlink(_) | InodeHeader::LSymlink(_) => symlinks += 1,
            InodeHeader::Dev(_) | InodeHeader::LDev(_) => devs += 1,
            InodeHeader::IPC(_) | InodeHeader::LIPC(_) => ipcs += 1,
        }
    }

    eprintln!(
        "inodes {}, directories {}, regulars {}, symlinks {}, devs {}, ipcs {}",
        inode_headers.len(), directories, regulars, symlinks, devs, ipcs
    );

    eprintln!(
        "fragments.len {}",
        fragments.len()
    );

    eprintln!(
        "lookup_table.len {}",
        lookup_table.len()
    );

    eprintln!(
        "id_table {:?}",
        id_table.ids()
    );

    // if let Some(_filename) = args.next() && let InodeHeader::Directory(_dir) = root_inode {
    //     let root = image.opendir(&dir)?;
    //     eprintln!("count {} inode number {}", root.count(), root.inode_number());
    //     let inodes = dir.inodes();
    //     for i in inodes {
    //         eprintln!("{}", i.count());
    //     }
    // };

    Ok(())
}

use std::fs::File;
use std::io::{Seek, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    max_size: u64,

    #[arg(short, long, default_value_t = false)]
    verbose: bool,

    #[arg(short, long)]
    output: PathBuf,

    #[arg()]
    files: Vec<PathBuf>,
}

fn flush_and_get_position(
    archive: &mut tar::Builder<brotli::CompressorWriter<&std::fs::File>>,
) -> Result<u64> {
    let compressor = archive.get_mut();
    compressor.flush().context("Could not flush output")?;

    Ok(compressor.get_mut().stream_position().unwrap())
}

fn add_manifest<W: Write>(args: &Args, archive: &mut tar::Builder<W>) -> Result<()> {
    let manifest =
        serde_json::to_value(&args.files).context("Could not create manifest")?.to_string();
    let manifest_data = manifest.as_bytes();

    let mut header = tar::Header::new_gnu();
    header.set_size(manifest_data.len() as u64);
    header.set_mode(0o644);

    archive
        .append_data(&mut header, "partial-tar-brotli-manifest.json", manifest_data)
        .context("Could not add manifest to archive")?;

    Ok(())
}

fn generate_archive_filename(orig: &Path) -> PathBuf {
    let mut res = std::path::PathBuf::new();

    for component in orig.components() {
        match component {
            std::path::Component::Normal(part) => {
                if !part.is_empty() {
                    res.push(part);
                }
            }
            std::path::Component::CurDir => (),
            std::path::Component::RootDir => {
                res.clear();
            }
            std::path::Component::ParentDir => {
                res.pop();
            }
            std::path::Component::Prefix(_) => todo!(),
        }
    }

    res
}

fn do_write(args: &Args) -> Result<()> {
    let mut out = File::create_new(&args.output).context("Could not create output file")?;

    let mut truncate_pos: Option<u64> = None;
    let mut added = 0;

    let mut archive = tar::Builder::new(brotli::CompressorWriter::new(&out, 4096, 11, 22));

    /* Don't need irrelevant details like timestamp and owner/group */
    archive.mode(tar::HeaderMode::Deterministic);

    add_manifest(args, &mut archive)?;

    for file in &args.files {
        let before_pos = flush_and_get_position(&mut archive)?;

        archive
            .append_path_with_name(file, generate_archive_filename(file))
            .context("Could not add file to archive")?;

        let after_pos = flush_and_get_position(&mut archive)?;
        if after_pos > args.max_size {
            if args.verbose {
                eprintln!("{} does not fit. Archive would be {} bytes.", file.display(), after_pos);
            }
            truncate_pos = Some(before_pos);
            break;
        }
        added += 1;
        if args.verbose {
            eprintln!("{} (used {} bytes)", file.display(), after_pos - before_pos);
        }
    }

    drop(archive);

    if let Some(p) = truncate_pos {
        /* Need to rewind (truncate) the archive to fit max-size.
         *
         * A flush() call has been made on the CompressorWriter so
         * that this position always ends a metadata block (and that
         * is at a byte boundary).
         *
         * All brotli files must end with a metadata block with the
         * "ISLAST" flag set. CompressorWriter::drop writes that
         * automatically, but that is lost when the file is
         * truncated. So it must be written manually here. Luckily
         * such empty last metadata block is really easy to write, as
         * it always constitutes a byte whose first two bits set
         * (ISLAST and ISLASTEMPTY). [RFC7932 9.2]
         *
         * Strictly speaking the tar file also should contain a end of
         * file marker (two blocks filled with 0x00) but at least GNU
         * tar ignores that (unless run with
         * `--warning=missing-zero-blocks` option)
         */
        out.set_len(p).unwrap();
        out.seek(std::io::SeekFrom::End(0)).expect("seek");
        out.write(&[0b0000_0011]).context("Could not write last byte to archive")?;
        eprintln!(
            "Done! {} out of {} files added ({} skipped)",
            added,
            args.files.len(),
            args.files.len() - added
        );
    } else {
        eprintln!("Done! All {} files added to archive.", added);
    }

    Ok(())
}

fn main() {
    if let Err(e) = do_write(&Args::parse()) {
        eprintln!("Error: {:?}", e);
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn test_generate_archive_filename() {
        fn check(orig: &str, exp: &str) {
            let p: PathBuf = orig.into();
            let expected: OsString = exp.into();

            let res = generate_archive_filename(&p);
            assert_eq!(res.as_os_str(), expected);

            // double normalization gives same
            assert_eq!(generate_archive_filename(&res), expected);
        }
        fn check_unchanged(path: &str) {
            check(path, path);
        }

        check_unchanged("test.txt");
        check_unchanged("foo/test.txt");
        check("foo//test.txt", "foo/test.txt");
        check("foo/test.txt//", "foo/test.txt");
        check("../some/file", "some/file");
        check("some/file/buried/../deep/down", "some/file/deep/down");
        check("/file/with/absolute/path", "file/with/absolute/path");
        check("/file/with/absolute/../path", "file/with/path");
        check("/../../crazy", "crazy");
    }
}

use indicatif::{ParallelProgressIterator, ProgressBar, ProgressStyle};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::path::Path;

pub enum MaxPieces {
    Three,
    Four,
    Five,
}

impl MaxPieces {
    pub const fn num(&self) -> u8 {
        match self {
            Self::Three => 3,
            Self::Four => 4,
            Self::Five => 5,
        }
    }
}

pub fn download_egtb<P: AsRef<Path> + Sync>(max: &MaxPieces, download_dir: P) {
    let (url, dir) = match max {
        MaxPieces::Three => (
            "https://syzygy-tables.info/download.txt?source=lichess&max-pieces=3",
            "3",
        ),
        MaxPieces::Four => (
            "https://syzygy-tables.info/download.txt?source=lichess&max-pieces=4",
            "3-4",
        ),
        MaxPieces::Five => (
            "https://syzygy-tables.info/download.txt?source=lichess&max-pieces=5",
            "3-4-5",
        ),
    };

    let body = reqwest::blocking::get(url).unwrap().text().unwrap();

    let urls: Vec<_> = body.lines().collect();

    std::fs::create_dir_all(download_dir.as_ref().join(dir)).unwrap();

    let bar = ProgressBar::new(urls.len() as u64);

    bar.set_style(
        ProgressStyle::with_template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>3}/{len:3}")
            .unwrap()
            .progress_chars("##-"),
    );

    urls.par_iter().progress_with(bar).for_each(|url| {
        let filename = url.split('/').last().unwrap();
        let path = download_dir.as_ref().join(filename);

        let body = reqwest::blocking::get(*url).unwrap().bytes().unwrap();

        std::fs::write(path, &body).unwrap();
    });

    println!(
        "Finished downloading max-{}-piece tablebases to {}",
        max.num(),
        download_dir
            .as_ref()
            .join(dir)
            .canonicalize()
            .unwrap()
            .display()
    );
}

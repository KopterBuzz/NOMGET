use env_logger::Builder;
use log::LevelFilter;
use std::io::Write;

pub fn init_logger(verbosity: u8, quiet: bool) {
    let mut builder = Builder::new();

    let level = if quiet {
        LevelFilter::Error
    } else {
        match verbosity {
            0 => LevelFilter::Info,
            1 => LevelFilter::Debug,
            _ => LevelFilter::Trace,
        }
    };

    builder
        .filter_level(level)
        .format(|buf, record| {
            let style = buf.default_level_style(record.level());
            writeln!(
                buf,
                "[{style}{}{style:#}] {}",
                record.level(),
                record.args()
            )
        })
        .init();
}

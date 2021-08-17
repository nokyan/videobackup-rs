extern crate clap;
extern crate num_cpus;

use clap::{Arg, App, SubCommand};

mod common;
mod decode;
mod encode;


static DISCLAIMER: &str = "IMPORTANT: THIS TOOL COMES WITH NO WARRANTY WHATSOEVER. USE AT YOUR OWN RISK.";

fn main() {
    let cpus = num_cpus::get().to_string();

    let matches = App::new("videobackup-rs")
            .version(env!("CARGO_PKG_VERSION"))
            .author("ManicRobot")
            .about("Turn any file into a video file and vice versa")
            .subcommand(App::new("encode")
                    .version(env!("CARGO_PKG_VERSION"))
                    .author("ManicRobot")
                    .about("Encodes a file into a video file")
                    .arg(Arg::with_name("INPUT")
                            .help("The file to be turned into a video")
                            .index(1)
                            .takes_value(true)
                            .multiple(false)
                            .required(true))
                    .arg(Arg::with_name("OUTPUT")
                            .help("The name of the result video")
                            .index(2)
                            .takes_value(true)
                            .multiple(false)
                            .required(true))
                    .arg(Arg::with_name("fps")
                            .long("fps")
                            .value_name("fps")
                            .help("FPS for the video, 6 is optimal")
                            .multiple(false)
                            .default_value("6")
                            .takes_value(true))
                    .arg(Arg::with_name("width")
                            .long("width")
                            .short("w")
                            .help("Width of the video")
                            .multiple(false)
                            .default_value("3840")
                            .takes_value(true))
                    .arg(Arg::with_name("height")
                            .long("height")
                            .short("h")
                            .help("Height of the video")
                            .multiple(false)
                            .default_value("2160")
                            .takes_value(true))
                    .arg(Arg::with_name("colors")
                            .long("colors")
                            .short("c")
                            .aliases(&["colours"])
                            .help("Amount of colors used. Less colors will take longer for encoding/decoding and make the file larger but the video will be more resistant against compression.")
                            .multiple(false)
                            .possible_values(&["2", "4"])
                            .default_value("2")
                            .takes_value(true))
                    .arg(Arg::with_name("bytes")
                            .long("ecc-bytes")
                            .short("e")
                            .help("Amount of ECC bytes in a 128-byte block. More bytes will make the file slightly larger, encoding/decoding times slightly longer but will massively improve resistance against compression.")
                            .multiple(false)
                            .default_value("16")
                            .takes_value(true))
                    // TODO: Reimplement custom codec
                    //.arg(Arg::with_name("codec")
                    //        .long("video-codec")
                    //        .short("vc")
                    //        .help("Tells ffmpeg which video encoder to use.")
                    //        .multiple(false)
                    //        .default_value("libx264")
                    //        .takes_value(true))
                    .arg(Arg::with_name("crf")
                            .long("crf")
                            .help("Quality of the video (constant rate factor). Lower values will increase quality (therefore less compression artifacts) and file size. Might not work with every video codec.")
                            .multiple(false)
                            .default_value("24")
                            .takes_value(true))
                    .arg(Arg::with_name("threads")
                            .long("threads")
                            .short("t")
                            .help("How many threads to use")
                            .multiple(false)
                            .default_value(&cpus)
                            .takes_value(true)))
            .subcommand(App::new("decode")
                    .version(env!("CARGO_PKG_VERSION"))
                    .author("ManicRobot")
                    .about("Decodes a video file into the original file")
                    .arg(Arg::with_name("INPUT")
                            .help("The file to be turned into a video")
                            .index(1)
                            .takes_value(true)
                            .multiple(false)
                            .required(true))
                    .arg(Arg::with_name("threads")
                            .long("threads")
                            .short("t")
                            .help("How many threads to use")
                            .multiple(false)
                            .default_value(&cpus)
                            .takes_value(true)))
            .get_matches();
    
    if let Some(ref matches) = matches.subcommand_matches("encode") {
        println!("→ videobackup-rs encoder {}", env!("CARGO_PKG_VERSION"));
        println!("ℹ {}", DISCLAIMER);
        encode::encode(matches.value_of("INPUT").unwrap(),
                       matches.value_of("OUTPUT").unwrap(),
                       matches.value_of("fps").unwrap().parse::<u16>().unwrap(),
                       matches.value_of("width").unwrap().parse::<usize>().unwrap(),
                       matches.value_of("height").unwrap().parse::<usize>().unwrap(),
                       matches.value_of("colors").unwrap().parse::<u16>().unwrap(),
                       matches.value_of("bytes").unwrap().parse::<u8>().unwrap(),
                       String::from("libx264"),
                       matches.value_of("crf").unwrap().parse::<u16>().unwrap(),
                       matches.value_of("threads").unwrap().parse::<usize>().unwrap());
    } else if let Some(ref matches) = matches.subcommand_matches("decode") {
        println!("→ videobackup-rs decoder {}", env!("CARGO_PKG_VERSION"));
        println!("ℹ {}", DISCLAIMER);
        decode::decode(matches.value_of("INPUT").unwrap(), true, matches.value_of("threads").unwrap().parse::<usize>().unwrap())
    } else {
        println!("⚠ Invalid subcommand! Use this command with --help for explanation!")
    }
}
extern crate crc32fast;
extern crate image;
extern crate path_absolutize;
extern crate positioned_io;
extern crate reed_solomon;

use crate::common::crc32_file;
use crate::common::LoHi;

use image::{ImageBuffer, RgbImage};

use path_absolutize::Absolutize;

use positioned_io::ReadAt;

use reed_solomon::Encoder;

use std::convert::TryInto;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::thread;


static ENCODING_VERSION: u16 = 3;

fn build_frame(bytes: &[u8], fps: u16, width: usize, height: usize, colors: u16, count: u32, video_codec: &str) -> String {
    // check whether someone supplied to many bytes for our image
    if bytes.len() as f32 > ((width * height) as f32) / ((colors as f32).log(256.0)) {
        panic!("Byte array is too large for the image size!")
    }

    // declare our color_palettes
    let two_color_palette = [image::Rgb([0, 0, 0]), image::Rgb([255, 255, 255])]; 
    let four_color_palette = [image::Rgb([0, 0, 0]), image::Rgb([255, 0, 0]), image::Rgb([0, 255, 0]), image::Rgb([0, 0, 255])];

    // construct a new image based on our width and height
    let mut image: RgbImage = ImageBuffer::new((width as usize).try_into().unwrap(), (height as usize).try_into().unwrap());
    
    // enumerate over byte array
    for i in 0..bytes.len() {
        if colors == 2 {
            // go through every bit of the current byte
            for j in 0..8 {
                // TODO: Optimize all those type conversions away
                // get the current pixel's position
                let pixel_x: u32 = ((i * 8 + j) % width).try_into().unwrap();
                let pixel_y: u32 = ((i * 8 + j) / width).try_into().unwrap();
                // get the j'th bit in the current byte
                let bit: usize = ((bytes[i] & ((128 / (u8::pow(2, j.try_into().unwrap()))) as u8)) >> (7-j)).into();
                // paint the "calculated" color to the image
                let pixel = two_color_palette[bit];
                image.put_pixel(pixel_x, pixel_y, pixel);
            }
        } else {
            panic!("Sorry, color palette sizes other than 2 are currently not implemented!")
        }
    }

    // we want to save the image in a tmp folder
    let img_path = Path::new("tmp").join(format!("{}.bmp", count)).absolutize().unwrap().to_str().unwrap().to_string();
    match image.save(&img_path) {
        Err(e) => println!("Error saving file #{}: {:?}", count, e),
        Ok(v) => (),
    }

    // convert it to an MPEG-TS using ffmpeg for easy stitching later
    let ts_path = Path::new("tmp").join(format!("{}.ts", count)).absolutize().unwrap().to_str().unwrap().to_string();
    let cmd_result = Command::new("ffmpeg")
                             .args(&["-y", "-r", &fps.to_string(), "-i", &img_path, "-t", &(1/fps).to_string(), "-c:v", video_codec, "-bsf:v", "h264_mp4toannexb", "-f", "mpegts", &ts_path])
                             .output();

    return ts_path;
}

fn prepare_build_image(bytes: &[u8], fps: u16, width: usize, height: usize, colors: u16, ecc_bytes: u8, count: u32, video_codec: &str) {
    
}

/// Encodes any file into a video.
pub fn encode(filename: &str, fps: u16, width: u32, height: u32, colors: u16, ecc_bytes: u8, video_codec: &str, crf: u16, threads: usize) -> String {

    // create temp folder for saving the BMP and TS files
    let res = std::fs::create_dir_all(Path::new("tmp"));
    match res {
        Err(e) => panic!("Unable to create temp folder! {}", e),
        Ok(v) => (),
    }

    // open the file and gather information for the metadata frame
    let file = fs::File::open(filename);
    match file {
        Err(e) => panic!("Unable to open file {}!", e),
        Ok(v) => (),
    }
    let file_metadata = fs::metadata(filename).unwrap();

    let file_size = file_metadata.len();

    let file_name = Path::new(filename).file_name().unwrap();
    if file_name.len() > 512 {
        panic!("The file name may not be longer than 512 characters!")
    }

    let crc32 = crc32_file(filename);

    // metadata looks like this:
    // - bytes 0-1 are the encoding version
    // - bytes 2-3 are the palette size
    // - byte 4 is the pixel size
    // - bytes 5-12 is the file size
    // - bytes 13-16 are the CRC32 checksum
    // - byte 17 is the amount of ECC bytes
    // - bytes 18-529 are the filename
    // - bytes 530-561 are the ECC for the metadata frame

    // make our metadata byte array for building the metadata frame
    let mut metadata_bytes: [u8; 561] = [0; 561];
    
    // TODO: build metadata frame, implement encoding of the actual file
    

    return Path::new(filename).absolutize().unwrap().to_str().unwrap().to_string();
}
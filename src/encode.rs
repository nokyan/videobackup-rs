extern crate crc32fast;
extern crate image;
extern crate path_absolutize;
extern crate reed_solomon;

use crate::common::BLOCK_SIZE;
use crate::common::ENCODING_VERSION;
use crate::common::crc32_file;
use crate::common::zero_vec;

use image::{ImageBuffer, RgbImage};

use path_absolutize::Absolutize;

use reed_solomon::Encoder;

use std::convert::TryInto;
use std::fs;
use std::io::prelude::*;
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};


fn build_frame(bytes: &[u8], fps: u16, width: usize, height: usize, colors: u16, count: u32, video_codec: String) -> std::path::PathBuf {
    // check whether someone supplied to many bytes for our image
    if bytes.len() as f32 > ((width * height) as f32) / (colors as f32).log(256.0) {
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
    let img_path = Path::new("tmp").join(format!("{}.png", count)).absolutize().unwrap().to_str().unwrap().to_string();
    match image.save(&img_path) {
        Err(e) => println!("Error saving file #{}: {:?}", count, e),
        Ok(v) => (),
    }

    // convert it to an MPEG-TS using ffmpeg for easy stitching later
    let ts_path = Path::new("tmp").join(format!("{}.ts", count));
    let cmd_result = Command::new("ffmpeg")
                             .args(&["-y", "-r", &fps.to_string(), "-i", &img_path, "-t", &(1.0f32/(fps as f32)).to_string(), "-c:v", &video_codec,
                              "-bsf:v", "h264_mp4toannexb", "-f", "mpegts", ts_path.to_str().unwrap()])
                             .output().unwrap();

    return ts_path;
}

/// This function takes a vector of byte arrays (the data part of the blocks), appends the ECC and then calls build_frame
fn prepare_build_image(bytes: Vec<Vec<u8>>, fps: u16, width: usize, height: usize, colors: u16, ecc_bytes: u8, count: u32, video_codec: String) -> std::path::PathBuf {
    // cbpf = content_bytes_per_frame, cbpb = content_bytes_per_block
    // the names were just too long
    let ecc_encoder = Encoder::new(ecc_bytes as usize);
    // initialize a vector with allocated space of blocks_per_frame * block_size, so basically the amount of bytes to be processed
    let mut bytes_for_frame: Vec<u8> = Vec::with_capacity(bytes.len() * BLOCK_SIZE as usize);
    for i in bytes.iter() {
        bytes_for_frame.extend_from_slice(i);
        bytes_for_frame.extend_from_slice(ecc_encoder.encode(i).ecc());
    }
    let result = build_frame(&bytes_for_frame[0..bytes_for_frame.len()], fps, width, height, colors, count, video_codec);
    return result;
}

/// Encodes any file into a video.
pub fn encode(input: &str, output: &str, fps: u16, width: usize, height: usize, colors: u16, ecc_bytes: u8, video_codec: String, crf: u16, threads: usize) -> String {

    let start_time = Instant::now();

    // create temp folder for saving the BMP and TS files
    let res = std::fs::create_dir_all(Path::new("tmp"));
    match res {
        Err(e) => panic!("Unable to create temp folder! {}", e),
        Ok(v) => (),
    }

    // open the file and gather information for the metadata frame
    let mut file = fs::File::open(input).unwrap();

    let file_metadata = fs::metadata(input).unwrap();

    let file_size = file_metadata.len();

    let file_name = Path::new(input).file_name().unwrap().to_str().unwrap();
    if file_name.len() > 200 {
        panic!("The input file name may not be longer than 200 characters!")
    }

    let crc32 = crc32_file(input);

    // calculate some geometry
    let blocks_per_frame = (((width * height) as f32) / (256f32).log(colors as f32) / (BLOCK_SIZE as f32)) as usize;
    let content_bytes_per_block = BLOCK_SIZE as usize - ecc_bytes as usize;
    let content_bytes_per_frame = blocks_per_frame * content_bytes_per_block;
    let needed_frames = ((file_size as f64 / content_bytes_per_frame as f64).ceil() as u64) + 1;

    println!("→ Starting videobackup-rs encoder with following parameters:");
    println!("  • FPS: {}", fps);
    println!("  • Width: {}", width);
    println!("  • Height: {}", height);
    println!("  • Colors: {}", colors);
    println!("  • ECC bytes: {}", ecc_bytes);
    println!("  • Video codec: {}", video_codec);
    println!("  • CRF: {}", crf);
    println!("  • Threads: {}", threads);
    println!("  • Needed frames: {}", needed_frames);

    // metadata looks like this:
    // - bytes 0-1 are the encoding version
    // - bytes 2-3 are the palette size
    // - byte 4 is the pixel size
    // - bytes 5-12 is the file size
    // - bytes 13-16 are the CRC32 checksum
    // - byte 17 is the amount of ECC bytes
    // - bytes 18-217 are the filename
    // - bytes 218-249 are the ECC for the metadata frame

    // make our metadata byte array for building the metadata frame
    let mut metadata_bytes: [u8; 250] = [0; 250];
    let ecc_encoder = Encoder::new(32);


    // start copying all the stuff over
    metadata_bytes[0..=1].copy_from_slice(&ENCODING_VERSION.to_be_bytes());
    metadata_bytes[2..=3].copy_from_slice(&colors.to_be_bytes());
    metadata_bytes[4] = 1; // TODO: change this when this tool allows for more than 2 colors
    metadata_bytes[5..=12].copy_from_slice(&file_size.to_be_bytes());
    metadata_bytes[13..=16].copy_from_slice(&crc32.to_be_bytes());
    metadata_bytes[17] = ecc_bytes;
    metadata_bytes[18..=(18 + file_name.len() - 1)].copy_from_slice(file_name.as_bytes());
    let ecc =  ecc_encoder.encode(&metadata_bytes[0..=217]);
    let bytes = ecc.ecc();
    metadata_bytes[218..=249].copy_from_slice(bytes);
    
    // TODO: build metadata frame, implement encoding of the actual file
    build_frame(&metadata_bytes, fps, width, height, 2, 0, video_codec);

    // rename our metadata frame video so we can start building the data frames
    match std::fs::rename(Path::new("tmp").join("0.ts"), Path::new("tmp").join("partial.ts")) {
        Ok(v) => (),
        Err(e) => panic!("Unable to rename the metadata frame to partial.ts! {}", e)
    }

    println!("→ Finished metadata frame; 1/{} ({:.1} %)", needed_frames, (100.0f32/needed_frames as f32));

    // read (content_bytes_per_frame * threads) bytes of data, slice it and send it to the threads
    let buffer_size = content_bytes_per_frame * threads;
    let mut read_bytes: Vec<u8> = zero_vec(buffer_size);
    let mut frame_count: u32 = 0;
    while let Ok(n) = file.read(&mut read_bytes[..]) {
        // this vector will at max contain (amount of threads) vectors of blocks that are ready for threads to chew through
        // it will probably contain less than (amount of threads) vectors when we reached EOF
        let mut prepared_frames: Vec<Vec<Vec<u8>>> = Vec::with_capacity(threads);
        let mut threads_to_use = threads;
        let mut reached_eof = false;
        if n != buffer_size {
            threads_to_use = ((n / content_bytes_per_frame) + 1).clamp(1, threads);
            reached_eof = true;
        } else {
            
        }

        // slice up frames for processing
        for i in 0..threads_to_use {
            let mut frame_vector: Vec<Vec<u8>> = Vec::with_capacity(blocks_per_frame);
            for j in 0..blocks_per_frame {
                let mut block: Vec<u8> = zero_vec(content_bytes_per_block);
                block.copy_from_slice(&read_bytes[(i * content_bytes_per_frame + j * content_bytes_per_block)..(i * content_bytes_per_frame + (j+1) * content_bytes_per_block)]);
                frame_vector.push(block);
            }
            prepared_frames.push(frame_vector);
        }

        // prepare some vectors and start multithreading
        let mut thread_handles = Vec::with_capacity(threads);
        let mut finished_frames: Vec<std::path::PathBuf> = Vec::new();
        let mut current_frame_count = 0;
        for p in prepared_frames {
            let handle = thread::spawn(move || {
                return prepare_build_image(p, fps, width, height, colors, ecc_bytes, current_frame_count, String::from("libx264"));
            });
            thread_handles.push(handle);
            current_frame_count += 1;
            frame_count += 1;
        }
        for t in thread_handles {
            finished_frames.push(t.join().unwrap());
        }

        // prepare the list for ffmpeg to concatenate the generated frames
        finished_frames.sort();
        finished_frames.insert(0, Path::new("tmp").join("partial.ts"));
        let mut list = String::new();
        for f in finished_frames.iter() {
            list.push_str("file ");
            list.push_str(&str::replace(f.to_str().unwrap(), "tmp/", ""));
            list.push_str("\n");
        }
        let mut list_txt = fs::File::create(Path::new("tmp").join("list.txt")).unwrap();
        list_txt.write(list.as_bytes()).unwrap();

        // do the ffmpeg concatenation of all ts files and then rename everything to how we started
        let con_result = Command::new("ffmpeg")
                                 .args(&["-y", "-f", "concat", "-r", &fps.to_string(), "-i", Path::new("tmp").join("list.txt").to_str().unwrap(), "-c", "copy", Path::new("tmp").join("new_partial.ts").to_str().unwrap()])
                                 .output().unwrap();

        std::fs::remove_file(Path::new("tmp").join("list.txt")).unwrap();
        std::fs::rename(Path::new("tmp").join("new_partial.ts"), Path::new("tmp").join("partial.ts")).unwrap();

        println!("→ Finished frames to {}/{} ({:.1} %)", frame_count + 1, needed_frames, ((((frame_count + 1) as f32 ) * 100.0f32)/needed_frames as f32));

        // some cleaning up
        read_bytes = zero_vec(buffer_size);
        if reached_eof {
            break;
        }
    }

    println!("→ Finishing the final video...");

    let final_cmd_result = Command::new("ffmpeg")
                             .args(&["-y", "-r", &fps.to_string(), "-i", Path::new("tmp").join("partial.ts").to_str().unwrap(), "-c", "copy", output])
                             .output().unwrap();

    println!("→ Cleaning up...");

    std::fs::remove_dir_all(Path::new("tmp")).unwrap();


    println!("→ Done in {} seconds!", (start_time.elapsed().as_millis() as f32 / 1000.0f32));

    return Path::new(input).absolutize().unwrap().to_str().unwrap().to_string();
}
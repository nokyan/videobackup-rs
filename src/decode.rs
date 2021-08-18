extern crate image;
extern crate path_absolutize;
extern crate reed_solomon;

use crate::common::BLOCK_SIZE;
use crate::common::ENCODING_VERSION;
use crate::common::crc32_file;
use crate::common::zero_vec;

use reed_solomon::Decoder;

use image::{GenericImageView};

use std::convert::TryInto;
use std::fs;
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;


/// extracts (amount) frames from the input video starting at (start)
fn get_frames(input: &str, start: u64, amount: u64) -> Vec<PathBuf> {
    let extract_res = Command::new("ffmpeg")
                              .args(&["-i", input, "-vf", &format!("select='between(n\\,{}\\,{})'", start, start+amount-1), "-vsync", "0", Path::new("tmp").join("%04d.png").to_str().unwrap()])
                              .output().unwrap();
    let mut return_vec: Vec<PathBuf> = Vec::with_capacity(amount as usize);
    // TODO: make that for loop more compact
    for i in 1..(amount+1) {
        return_vec.push(Path::new("tmp").join(format!("{:0>4}.png", i)));
    }
    return return_vec;
}

/// deletes the temporarily extracted frames
fn delete_frames(amount: u64) {
    for i in 1..(amount+1) {
        fs::remove_file(Path::new("tmp").join(format!("{:0>4}.png", i))).unwrap();
    }
}

/// failsafe for trying to read colors that aren't in the color palette
fn try_read_pixel(color: [u8; 3], color_palette: &Vec<[u8; 3]>) -> (usize, bool) {
    let index = color_palette.iter().position(|&r| r == color);
    match index {
        // the color is actually in the color palette, return that
        Some(v) => { return (v, true); },
        // the color isn't in the color palette, search the color in the color palette nearest to our color
        None => {
            // TODO: make this more efficient
            // we create a vector (not a hashmap because we need sorting) where we map each color of the color palette
            // to the distance from the color we read, sort it and then look what color is closest
            let mut unsorted_distances: Vec<(usize, i32)> = Vec::with_capacity(color_palette.len());
            for i in 0..color_palette.len() {
                let distance: i32 = ((color[0] as i32 - color_palette[i][0] as i32) * (color[0] as i32 - color_palette[i][0] as i32))
                                  + ((color[1] as i32 - color_palette[i][1] as i32) * (color[1] as i32 - color_palette[i][1] as i32))
                                  + ((color[2] as i32 - color_palette[i][2] as i32) * (color[2] as i32 - color_palette[i][2] as i32));
                unsorted_distances.push((i, distance));
            }
            unsorted_distances.sort_by(|a, b| a.1.cmp(&b.1));
            return (unsorted_distances[0].0, false);
        }
    }
}

/// tries to read a single frame
fn read_raw_frame(input: &str, colors: u16) -> (Vec<u8>, u128, u128, u32, u32) {
    let two_color_palette: Vec<[u8; 3]> = vec![[0, 0, 0], [255, 255, 255]]; 
    let four_color_palette: Vec<[u8; 3]> = vec![[0, 0, 0], [255, 0, 0], [0, 255, 0], [0, 0, 255]];

    let mut correct_pixels: u128 = 0;
    let mut estimated_pixels: u128 = 0;

    let img = image::open(input).unwrap();

    let mut buf: Vec<u8> = Vec::with_capacity((((img.dimensions().0 * img.dimensions().1) as f32) / (colors as f32).log(256.0)) as usize);

    let mut current_byte: u8 = 0;

    for (i, pixel) in img.pixels().enumerate() {
        let read_color = [pixel.2[0], pixel.2[1], pixel.2[2]];
        if colors == 2 {
            let read_pixel = try_read_pixel(read_color, &two_color_palette);
            // OR the read bit (since we're in 2 color mode) with the currently read byte
            current_byte = current_byte | ((read_pixel.0 as u8) << (7 - (i % 8)));
            if i % 8 == 7 {
                // we've written all 8 bits for our byte, push it to the buffer and start reading
                // a new one next time
                buf.push(current_byte);
                current_byte = 0;
            }
            if read_pixel.1 {
                correct_pixels += 1;
            } else {
                estimated_pixels += 1;
            }
        } else if colors == 4 {
            let read_pixel = try_read_pixel(read_color, &four_color_palette);
            // OR the read 2 bits (since we're in 4 color mode) with the currently read byte
            current_byte = current_byte | ((read_pixel.0 as u8) << (6 - ((i % 4) * 2)));
            if i % 4 == 3 {
                // we've written all 8 bits for our byte, push it to the buffer and start reading
                // a new one next time
                buf.push(current_byte);
                current_byte = 0;
            }
            if read_pixel.1 {
                correct_pixels += 1;
            } else {
                estimated_pixels += 1;
            }
        }
    }
    return (buf, correct_pixels, estimated_pixels, img.dimensions().0, img.dimensions().1);
}

/// This function calls read_raw_frame and then handles the ECC stuff
fn read_frame(frame: &str, colors: u16, ecc_count: u8, blocks_per_frame: usize, number: u64) -> (Vec<u8>, u128, u128, u64, u64) {
    let content_bytes_per_block: usize = (BLOCK_SIZE - ecc_count) as usize;
    let content_bytes_per_frame = blocks_per_frame * content_bytes_per_block;
    let mut buf: Vec<u8> = Vec::with_capacity(blocks_per_frame * content_bytes_per_block as usize);

    let decoder = Decoder::new(ecc_count as usize);
    let mut ecced_bytes: u64 = 0;
    let mut unrecoverable_blocks: u64 = 0;

    let frame = read_raw_frame(frame, colors);
    
    for i in 0..blocks_per_frame {
        let current_block = &frame.0[(i * BLOCK_SIZE as usize)..((i+1) * BLOCK_SIZE as usize)];
        let decoded_bytes = decoder.correct_err_count(current_block, None);
        match decoded_bytes {
            Ok(v) => {
                buf.extend_from_slice(v.0.data());
                ecced_bytes += v.1 as u64;
            },
            Err(e) => {
                buf.extend_from_slice(&current_block[0..content_bytes_per_block]);
                println!("⚠ WARNING: Encountered an unrecoverable data block starting at {:#X} and ending at (not including) {:#X}. The block will be inserted without any error correction, your file is very likely to be damaged.", 
                         (number * content_bytes_per_frame as u64 + i as u64 * content_bytes_per_block as u64),
                         (number * content_bytes_per_frame as u64 + (i+1) as u64 * content_bytes_per_block as u64));
                unrecoverable_blocks += 1;
            }
        }
    }
    return (buf, frame.1, frame.2, ecced_bytes, unrecoverable_blocks);
}

pub fn decode(input: &str, checksum: bool, threads: usize) {
    let start_time = Instant::now();

    // create temp folder for saving the PNG and TS files
    std::fs::create_dir_all(Path::new("tmp")).unwrap();

    // we want to have some metrics for the end
    let mut correct_pixels: u128 = 0;
    let mut estimated_pixels: u128 = 0;
    let mut ecced_bytes: u64 = 0;
    let mut unrecoverable_blocks: u64 = 0;

    println!("→ Starting videobackup-rs decoder");

    // get the number of frames in the video
    println!("→ Counting frames in video...");
    let mut frames_amount: u64 = 0;
    let first_ffprobe_res = Command::new("ffprobe")
                           .args(&["-v", "error", "-select_streams", "v:0", "-show_entries", "stream=nb_frames", "-of", "default=nokey=1:noprint_wrappers=1", input])
                           .output().unwrap();
    let first_ffprobe_int = String::from_utf8(first_ffprobe_res.stdout).unwrap().replace("\n", "").parse::<u64>();
    match first_ffprobe_int {
        Ok(v) => { frames_amount = v },
        Err(e) => {
            println!("→ Unable to use container information to get frames count, resorting to manually counting (this will take a while)...");
            let second_ffprobe_res = Command::new("ffprobe")
                                    .args(&["-v", "error", "-count_frames", "-select_streams", "v:0", "-show_entries", "stream=nb_read_frames", "-of", "default=nokey=1:noprint_wrappers=1", input])
                                    .output().unwrap();
            frames_amount = String::from_utf8(second_ffprobe_res.stdout).unwrap().replace("\n", "").parse::<u64>().unwrap();
        }
    }
    println!("→ Counted {} frames", frames_amount);

    // decode the metadata frame
    let metadata_path = &get_frames(input, 0, 1)[0];
    let metadata_frame = read_raw_frame(metadata_path.to_str().unwrap(), 2);     // reminder: the metadata frame *always* has 2 colors
    let metadata_ecc_decoder = Decoder::new(32);

    let metadata_bytes = &metadata_frame.0[0..250];
    let metadata_ecc = metadata_ecc_decoder.correct(metadata_bytes, None).unwrap();
    let metadata = metadata_ecc.data();
    let encoding_version = u16::from_be_bytes(metadata[0..=1].try_into().unwrap());
    let colors = u16::from_be_bytes(metadata[2..=3].try_into().unwrap());
    let pixel_size = metadata[4];
    let file_size = u64::from_be_bytes(metadata[5..=12].try_into().unwrap());
    let crc32_checksum = u32::from_be_bytes(metadata[13..=16].try_into().unwrap());
    let ecc_bytes = metadata[17];
    let file_name = String::from_utf8(metadata[18..=217].to_vec()).unwrap().replace("\0", "");
    let width = metadata_frame.3;
    let height = metadata_frame.4;
    let content_bytes_per_block: usize = (BLOCK_SIZE - ecc_bytes) as usize;
    let blocks_per_frame = (((width * height) as f32) / (256f32).log(colors as f32) / (BLOCK_SIZE as f32)) as usize;

    delete_frames(1);

    println!("→ Successfully read metadata frame; 1/{} ({:.1} %)", frames_amount, (100.0f32/frames_amount as f32));
    println!("→ The file has the following properties:");
    println!("  • Name: {}", file_name);
    println!("  • Size: {} Bytes", file_size);
    println!("  • CRC32: {}", crc32_checksum);
    println!("  • Encoding version: {}", encoding_version);

    // we're not compatible with files that were encoded with a different version
    if encoding_version < ENCODING_VERSION {
        panic!("Encoding version of {} is not compatible with this videobackup version's encoding version ({}). Obtain an earlier version of videobackup and try again.", input, ENCODING_VERSION);
    }
    if encoding_version > ENCODING_VERSION {
        panic!("Encoding version of {} is not compatible with this videobackup version's encoding version ({}). Obtain a newer version of videobackup and try again.", input, ENCODING_VERSION);
    }

    // prepare multithreading fun by generating the arguments for frame extracting
    // since it's likely that the number of frames (without metadata) is not cleanly divisible by (threads),
    // we have to watch out for not trying to read non-existing frames at the end
    let mut arguments: Vec<(u64, u64)> = Vec::new();
    let full_runs: u64 = (frames_amount-1) / threads as u64; // -1 because we don't want to add the metadata frame to the actual file
    let last_run: u64 = (frames_amount-1) % threads as u64;
    for i in 0..full_runs {
        arguments.push((i as u64 * threads as u64 + 1, threads as u64));
    }
    if last_run != 0 {
        arguments.push((frames_amount - last_run, last_run))
    }

    let mut frame_counter: u64 = 0;

    let mut file = fs::File::create(Path::new(&file_name)).unwrap();

    // multithreading fun!
    for i in arguments {
        let frames = get_frames(input, i.0, i.1);

        let mut thread_handles = Vec::with_capacity(threads);
        let mut buf: Vec<(u64, (Vec<u8>, u128, u128, u64, u64))> = Vec::new();
        
        let mut current_frame_counter: u64 = 0;
        for f in frames {
            let handle = std::thread::spawn(move || {
                return read_frame(f.to_str().unwrap(), colors, ecc_bytes, blocks_per_frame, frame_counter);
            });
            thread_handles.push((current_frame_counter, handle));
            current_frame_counter += 1;
            frame_counter += 1;
        }
        for t in thread_handles {
            buf.push((t.0, t.1.join().unwrap()));
        }

        // threads probably won't finish in order, so let's sort them
        buf.sort_by(|a, b| a.0.cmp(&b.0));

        for b in buf {
            file.write(&b.1.0).unwrap();
            correct_pixels += b.1.1;
            estimated_pixels += b.1.2;
            ecced_bytes += b.1.3;
            unrecoverable_blocks += b.1.4;
        }

        println!("→ Decoded frames to {}/{} ({:.1} %)", frame_counter + 1, frames_amount, ((((frame_counter + 1) as f32 ) * 100.0f32)/frames_amount as f32));

        delete_frames(i.1);
    }

    // the last frame probably contains a bunch of useless NULs
    println!("→ Truncating trailing NULs...");
    file.set_len(file_size).unwrap();

    if checksum {
        println!("→ Checking CRC32...");
        let crc32_end = crc32_file(&file_name);
        if crc32_checksum == crc32_end {
            println!("→ CRC32 check successful!")
        } else {
            println!("⚠ CRC32 check unsuccessful! Your file is likely corrupted!")
        }
    }

    println!("→ Cleaning up...");
    std::fs::remove_dir_all(Path::new("tmp")).unwrap();

    println!("✓ Done in {} seconds!", (start_time.elapsed().as_millis() as f32 / 1000.0f32));
    let guessed_percentage: f32 = (estimated_pixels as f32 * 100.0f32) / (estimated_pixels + correct_pixels) as f32;
    let ecced_percentage: f32 = (ecced_bytes as f32 * 100.0f32) / file_size as f32;
    println!("  • Total pixels: {} - Guessed pixels: {} - Perfectly read pixels: {} - Percentage of guessed pixels: {:.1} %", estimated_pixels + correct_pixels, estimated_pixels, correct_pixels, guessed_percentage);
    println!("  • Total bytes: {} - Unrecoverable bytes: {} - ECC'ed bytes: {} - Perfectly read bytes: {} - Percentage of ECC'ed bytes: {:.1} %", file_size, unrecoverable_blocks as usize * content_bytes_per_block, ecced_bytes, file_size - ecced_bytes, ecced_percentage);
}
# videobackup-rs

A port of [videobackup](https://github.com/ManicRobot/videobackup) (Python scripts that can turn any file into a video file and vice versa) to Rust

## About this tool

**videobackup-rs** can encode any file into a "normal" video and vice versa. This can be used to upload files of any format to platforms that usually only allow video. This comes with quite a large overhead in both encoding/decoding time and additionally a resulting video file size that is often 5 to 9 times as large as the original file.

## Why port a working tool to Rust?

Mainly for trying to improve memory efficiency (the original set of Python scripts used up ~700 MiB of memory per thread during decoding) and encoding/decoding time and also as a way for me to give Rust a proper try.

## Requirements

All Rust dependencies are handled in the ``Cargo.toml``.

You will also need [``ffmpeg``](https://www.ffmpeg.org/) along with ``ffprobe`` installed for your computer as this script needs it for extracting frames from videos and stitching together video files.

You'll also want quite beefy hardware especially when it comes to CPU, since this tool can and will (unless you don't want it to of course) make use of as many threads as possible.

## Usage

For very basic (and usually sufficient usage), you use this to encode any file (let's call that file ``important_document.pdf``) to a video file (you can choose its name, for here, let's call it ``document.mp4``):

```./videobackup encode important_document.pdf document.mp4```

If you want to decode it back to ``important_document.pdf`` again, you use this:

```./videobackup decode document.mp4```

The tool remembers the name of the original file, so there's no need to type it again when decoding.

It's recommended to use MP4 as container for the video file since many other containers like FLV and MKV apparently don't save information about the number of frames in the video and you don't want to suffer through ffmpeg having to manually count the frames as that takes quite long.

videobackup-rs does currently not quite have feature parity with videobackup, so when encoding, you can give it the following command line arguments:

- ``--fps <N>`` - FPS for the video, 6 is optimal for YouTube and is also default.
- ``--width <N>`` - width of the video
- ``--height <N>`` - height of the video
- ``--colors <N>`` - amount of colors used. Less colors will take longer for encoding/decoding and make the file larger but the video will be more resistant against compression, default is 2.
- ``--ecc_bytes <N>`` - amount of ecc bytes in a 128-byte block. More bytes will make the file slightly larger, encoding/decoding times slightly longer but will massively improve resistance against compression.
- ``--video_codec <codec>`` - tells ffmpeg which video encoder to use. Default is libx264.
- ``--crf <N>`` - quality of the video (constant rate factor). *Lower* values will increase quality (therefore less compression artifacts) and file size. Might not work with every video codec. Default is 24.
- ``--threads <N>`` - how many threads to use. Default is as many as your CPU has.

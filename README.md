# soft_matrix

Soft Matrix upmixes two-channel stereo to surround sound.

## Goals and Purpose

The goal of Soft Matrix is to provide ideal upmixing of two-channel stereo audio to 5.1 channels. Positioning of sounds are based on their panning between the two channels and the phase difference between two channels.

Soft Matrix's default matrix works very well with recordings that have significant out-of-phase material, and Soft Matrix has a horseshoe mode for recordings with significant panning; but mostly in-phase material.

Currently, Soft Matrix supports the [RM](https://en.wikipedia.org/wiki/QS_Regular_Matrix) matrix. The goal is to support [common phase and panning based matrixes](https://en.wikipedia.org/wiki/Matrix_decoder), including [SQ](https://en.wikipedia.org/wiki/Stereo_Quadraphonic) and [Dolby Stereo](https://en.wikipedia.org/wiki/Dolby_Stereo#The_Dolby_Stereo_Matrix).

## Usage

To use Soft Matrix with default options, merely run:

    soft_matrix input.wav output.wav

More options and examples are described in [options.md](options.md).

Soft Matrix only supports wav files as inputs. It only outputs 32-bit floating point wav files. (I reccomend [sox](https://sox.sourceforge.net/) for converting to/from wav.)

## Installation

Soft Matrix is only available as source code. It is written in Rust.

### Pre-requisites

1. Install Rust: <https://www.rust-lang.org/tools/install>
2. Install Git: <https://git-scm.com/book/en/v2/Getting-Started-Installing-Git>

### Building and Running

Once you have installed Rust and git:

    git clone https://github.com/GWBasic/soft_matrix.git
    cd soft_matrix
    cargo build --release

The soft_matrix binary will be in the soft_matrix/target/release folder:

    cd target/release
    ./soft_matrix

Due to the early status of this project, there currently are no pre-built binaries available; nor is soft_matrix available on systems like Chocolatey or Homebrew.

### Supported Platforms

**I currently develop on Mac.** Soft Matrix successfully runs on both Intel and Apple silicon.

I have not tested on Windows or Linux yet; but I am optimistic that Soft Matrix will build and run on those platforms.

## Tips

_When upmixing a continuous performance, you will have best results if all tracks are concatinated into a single file._ (For example, if you upmix the second side of [Abbey Road](https://en.wikipedia.org/wiki/Abbey_Road), concatinate it into a single wav file.) This is because the upmixer inspects roughly 1/20th second of audio at a time. If there are file breaks throughout a continuous performance, it will interfere with [windowing](https://en.wikipedia.org/wiki/Window_function) and could lead to a noticable click at the track break.

I personally use [sox](https://sox.sourceforge.net/) for converting between different audio formats, like wav and flac.

## How does it Work?

Soft Matrix attempts to steer audio by:

1. Breaking each sample up into its frequency components
2. Steering based on the instantanious panning and phase relationship between each frequency component in each sample

To do this, Soft Matrix performs a fourier transform for each sample in the source wav file. It uses a window size large enough to process down to 20hz. To prevent noise, panning from adjacent samples are averaged.

## Performance and Speed

Soft Matrix runs slowly. On my M2 Macbook Pro, it generally can upmix in approximately realtime.

This is because

- Fourier transforms large enough to go down to 20hz take a long time to perform.
- Soft Matrix performs a transform for every sample.
- Soft Matrix performs significant averaging of adjacent panning calculations.

To make Soft Matrix run as quickly as possible, it uses all available cores.

In order to keep performance "within reason," I suggest avoiding unreasonable sampling rates. Human hearing, in rare circumstances, [only goes up to 28khz](https://en.wikipedia.org/wiki/Hearing_range#Humans). Therefore, if you use high sampling rates, I suggest downsampling to 56khz before using Soft Matrix.

[Options.md](options.md) lists some other tuning options for faster upmixes; but I only reccomend them for previews.

## Feature Requests

Currently, I only plan on adding features to support additional out-of-phase matrixes, performance, audio quality, and configuration options.

_I do not plan on adding any other features._

Specifically, I have no plans to support reading other file formats, outputting to other file formats, or outputting anything other than a 32-bit wav file. There are many excellent tools for audio format conversion that can handle this much better than I can. I personally use [sox](https://sox.sourceforge.net/).

## Getting Help

If you need assistance, please visit <https://andrewrondeau.com/blog/> and email me directly.

Please understand that:

- You should use [Google](https://www.google.com/) or your favorite seach engine before asking for help.
- This is a hobby project; it may take me some time to respond.
- I can not provide help with other audio processing tools, like sox.

## Contributing

If you would like to contribute, please contact me using the above channels so that we may discuss your goals and motivations.

I would really appreciate help distributing through tools like Homebrew and Chocolatey.

## License

Soft Matrix is distributed under the [MIT license](LICENSE)

# soft_matrix

Soft Matrix upmixes two-channel stereo to surround sound.

## Goals and Purpose

The goal of Soft Matrix is to provide ideal upmixing of two-channel stereo audio to 5.1 channels. Positioning of sounds are based on their panning between the two channels and the phase difference between two channels.

Soft Matrix's default matrix works very well with recordings that have significant out-of-phase material, and Soft Matrix has a horseshoe mode for recordings with significant panning; but mostly in-phase material.

Currently, Soft Matrix supports the [RM](https://en.wikipedia.org/wiki/QS_Regular_Matrix) and [Dolby Stereo](https://en.wikipedia.org/wiki/Dolby_Stereo#The_Dolby_Stereo_Matrix) matrixes. The goal is to support [common phase and panning based matrixes](https://en.wikipedia.org/wiki/Matrix_decoder), including [SQ](https://en.wikipedia.org/wiki/Stereo_Quadraphonic). (Current support for SQ is experimental.)

## Usage

To use Soft Matrix with default options, merely run:

    soft_matrix "input.wav" "output.wav"

More options and examples are described in [options.md](options.md).

Soft Matrix only supports wav files as inputs. It only outputs 32-bit floating point wav files. (I recommend [sox](https://sox.sourceforge.net/) for converting to/from wav.)

## Installation

Soft Matrix is available via cargo, or as source code. It is written in Rust.

### Installation via Cargo

Prerequisite: [Install Rust](https://www.rust-lang.org/tools/install)

    cargo install soft_matrix

To update, merely re-run the above command.

### Chocolatey and Homebrew support?

There are currently open "help wanted" issues to support Chocolatey and Homebrew:
- Chocolatey (Windows): https://github.com/GWBasic/soft_matrix/issues/81
- Homebrew (Homebrew): https://github.com/GWBasic/soft_matrix/issues/80

### Pre-built binaries?

There currently are no plans to provide pre-built binaries.

### Building and Running from Source Code

Once you have [installed Rust](https://www.rust-lang.org/tools/install) and [installed Git](https://git-scm.com/book/en/v2/Getting-Started-Installing-Git):

    git clone https://github.com/GWBasic/soft_matrix.git
    cd soft_matrix
    cargo build --release

The soft_matrix binary will be in the soft_matrix/target/release folder:

    cd target/release
    ./soft_matrix

### Supported Platforms

**I currently develop on Mac.** Soft Matrix successfully runs on both Intel and Apple silicon.

I have not tested on Windows or Linux yet; but I am optimistic that Soft Matrix will build and run on those platforms.

## Examples

Examples are in [options.md](options.md)

## Examples (for sox)

### Convert a flac (or mp3) to a wav file

    sox "spiral.flac" "spiral.wav"

### Convert a wav to a flac file

Note that soft_matrix's output is a 32-bit floating point wav. This is a very inefficient file format, even compared to a 24-bit flac.

24-bit flac file: (Blu-ray, master quality)
    sox "spiral - upmixed.wav" -b 24 "spiral - upmixed.flac" dither -s -p 24

16-bit flac file: (CD quality)
    sox "spiral - upmixed.wav" -b 16 "spiral - upmixed.flac" dither -s -p 16

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

This is because:
- Fourier transforms large enough to go down to 20hz take a long time to perform.
- Soft Matrix performs a transform for every sample.
- Soft Matrix performs significant averaging of adjacent panning calculations.

To make Soft Matrix run as quickly as possible, it uses all available cores.

To keep performance "within reason," I suggest avoiding unreasonable sampling rates. Human hearing, in rare circumstances, [only goes up to 28khz](https://en.wikipedia.org/wiki/Hearing_range#Humans). Therefore, if you use high sampling rates, I suggest downsampling to 56khz before using Soft Matrix.

[Options.md](options.md) lists some other tuning options for faster upmixes; but I only recommend them for previews.

## Feature Requests

Currently, I only plan on adding features to support additional out-of-phase matrixes, performance, audio quality, and configuration options.

_I do not plan on adding any other features._

Specifically, I have no plans to support reading other file formats, outputting to other file formats, or outputting anything other than a 32-bit wav file. There are many excellent tools for audio format conversion that can handle this much better than I can. I personally use [sox](https://sox.sourceforge.net/).

## Getting Help

Before asking for help:

- Use [Google](https://www.google.com/) or your favorite search engine before asking for help; especially with issues related to Git or Rust.
- This is a hobby project; it may take me some time to respond.
- I cannot provide help with other audio processing tools, like sox.

If you need assistance, please visit <https://andrewrondeau.com/blog/> and email me directly.

## SQ matrix

Current support for the SQ matrix is limited. Positioning within the SQ matrix is approximate; panning levels are incorrect and there may be noise or other distortion. I do not recommend using soft_matrix for professional SQ dematrixing.

The SQ matrix is a very unusual matrix compared to typical phase-based matrixes like Dolby Surround, RM, and the default matrix. These all work by maintaining the same left-to-right pan and using phase to pan front to back. (Tones that are in-phase (0 degrees phase difference) are in the front, and tones that are out-of-phase (180 degree phase difference) are in the back.) Instead, the SQ matrix uses phase to steer a tone around the perimeter of the room as if it's a circle.

I personally spent at least 6 months of weekends trying to get SQ "right." Unfortunately, SQ relies on trigonometry that I really struggle with.

## Source Separation (Stemming)

[Source Separation](https://en.wikipedia.org/wiki/Computer_audition#Source_separation) (Stemming) is the act of separating out individual channels from a fully-mixed recording. It is the technology used to finish The Beatles' [Now and Then](https://en.wikipedia.org/wiki/Now_and_Then_(Beatles_song)#MAL_restoration_and_final_version).

soft_matrix does not perform any source separation. I am unfamiliar with source separation tools, but if you'd like to use them, I suggest:
1. Do source separation before using soft_matrix.
2. Each separated source should be stereo, and preserve the phase of the original recording
3. Use soft_matrix separately on each source
4. Mix all upmixes together

Please get in touch with me if you do this. I am interested in hearing if it works!

## Contributing

If you would like to contribute, please contact me using the above channels so that we may discuss your goals and motivations.

I would really appreciate help distributing through tools like Homebrew and Chocolatey. If you are motivated to upmix some SQ recordings, and enjoy math, maybe we can figure out SQ.

## License

Soft Matrix is distributed under the [MIT license] (LICENSE.md)



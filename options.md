# Options in Soft Matrix

Soft Matrix has a few options for configuring the generated wave file and how it processes sound.

## Output Options

**-matrix**: Chooses the matrix to use. Available matrixes are:

- **default**: The default matrix, used when the "-matrix" option is omitted. Sounds that are out-of-phase are panned to the rear. Sounds that are in phase are panned to the front. A good "all-round" matrix for recordings with significant out-of-phase material.
- **horseshoe**: Intended for recordings that are mostly panned between the two speakers, without much out-of-phase material. Widening is applied, and sounds that are in the extreme right and left are panned to the rear. Out-of-phase material is also panned to the rear.
- **dolby**: Adheres to the Dolby Stereo matrix, also known as LtRt, Dolby Surround, and Dolby Pro-Logic. Dolby Stereo was used on analog soundtracks for theatrical movies starting in the late 1970s, and was also used in analog television. When recordings encoded in Dolby Digital are downmixed to stereo, they are often matrixed using Dolby Stereo. (See <https://en.wikipedia.org/wiki/Dolby_Stereo#The_Dolby_Stereo_Matrix> for more information.)
- **qs**: Adheres to the "qs" matrix. Very similar to the "default" matrix, except that some minor widening is present. (See <https://en.wikipedia.org/wiki/QS_Regular_Matrix> for more information.)
- **rm**: Synonym for "qs". This option exists because it was common to mislabel qs-encoded recordings as rm.
- **sq**: EXPERIMENTAL! Adheres to the "sq" matrix. Although this matrix had a lot of commercial releases in the late 1970s, its technical limitations held it back from widespread adoption. Due to SQ's flaws, this option should only be used on material explicitly encoded for SQ. (See <https://en.wikipedia.org/wiki/Stereo_Quadraphonic>). (Note that sq support is experimental. This approach closely inspects phase and amplitude, but doesn't decode very well.)
- **sqexperimental**: An experimental decoder for sq that preserves in-phase front tones very well, and then uses a "by the book" dematrixer when
tones aren't in phase. This also works poorly. It may be removed in a future release of Soft Matrix.

**-channels**: The channel layout in the output file

- **4**: Four-channel layout; quadraphonic. Includes front right and left; and rear front and left.
- **5**: Five-channel layout. Includes front right, center, and left; and rear front and left.
- **5.1**: Five-point-one channel layout. Includes front right, center, and left; rear front and left; and a subwoofer channel.

**-minimum**: The minimum amplitude to steer front-to-back. Defaults to 0.01. On very clean signals, it may be useful to use a lower
threshold, like 0.0001. (This is needed because sounds that are isolated into the right front or right left speaker may be mis-steered due to the phase of noise in the adjacent source channel.)

**-loud**: Does not lower the amplitude when generating a center or LFE channel. [Because a center or LFE channel is based off of mixing the right and left channels, the overall amplitude is lowered in order to avoid clipping.](<Documentation/The loud flag.md>) This setting is useful when upmixing source material that is quiet, or otherwise mixed in a way to prevent clipping when upmixed. (Upmixing to 4.0 defaults to loud). (Not valid for 4.0.)

**-quiet**: Lowers the amplitude. (Default behavior for 4.1, 5.0, and 5.1.)

## Performance Options

**-low**: Specifies the lowest frequency calculated in the matrix. (Defaults to 20 hz.) Steering lower frequencies will make Soft Matrix run very slowly. If this is set too high, it may impede calculating the subwoofer or steering audible frequencies. (Very low frequencies require a much larger window for Fourier transforms. Larger windows take significantly longer to calculate.)

**-threads**: The number of threads to run. Defaults to [available_parallelism()](https://doc.rust-lang.org/stable/std/thread/fn.available_parallelism.html). This option is useful because available_parallelism() may return a number lower than the number of cores present in the CPU. Setting this higher than the number of cores in your CPU is not advised. This is a useful option if soft_matrix makes your computer run slowly.

**-keepawake**: Controls if soft_matrix keeps the computer awake. When true, the computer is prevented from sleeping while soft_matrix is running. When false, the computer can sleep while idle. Defaults to true.

## Examples

### Upmix a wave file using all defaults

    soft_matrix "stereo.wav" "surround.wav"

This will upmix stereo.wav, the input file, to a 5.1 wav file named surround.wav, using the default matrix.

### Use the RM matrix

    soft_matrix "stereo.wav" "surround.wav" -matrix rm

This will upmix stereo.wav, the input file, to a 5.1 wav file named surround.wav, using the RM matrix.

### Only run a single thread

    soft_matrix "stereo.wav" "surround.wav" -threads 1

This will upmix stereo.wav, the input file, using only a single thread. This option is useful if soft_matrix is making your computer run slowly.

### Run a fast encode without an LFE channel

    soft_matrix "stereo.wav" "preview.wav" -low 60 -channels 5

This only steers frequencies above 60 hz. Useful for a quick preview of upmixing.

### Allow the computer to sleep while upmixing

    soft_matrix "stereo.wav" "surround.wav" -keepawake false

This will allow the computer to sleep while upmixing. (Default behavior is that the computer will not sleep while running.)

# Options in Soft Matrix

Soft Matrix has a few options for configuring the generated wave file and how it processes sound.

## Output Options

**-matrix**: Chooses the matrix to use. Available matrixes are:

- **default**: The default matrix, used when the "-matrix" option is omitted. Sounds that are out-of-phase are panned to the rear. Sounds that are in phase are panned to the front. A good "all-round" matrix for recordings with significant out-of-phase material.
- **horseshoe**: Intended for recordings that are mostly panned between the two speakers, without much out-of-phase material. Widening is applied, and sounds that are in the extreme right and left are panned to the rear. Out-of-phase material is also panned to the rear.
- **dolby**: Adheres to the Dolby Stereo matrix, also known as LtRt, Dolby Surround, and Dolby Pro-Logic. Dolby Stereo was used on analog soundtracks for theatrical movies starting in the late 1970s, and was also used in analog television. When recordings encoded in Dolby Digital are downmixed to stereo, they are often matrixed using Dolby Stereo. (See <https://en.wikipedia.org/wiki/Dolby_Stereo#The_Dolby_Stereo_Matrix> for more information.)
- **dolbyloud**: Similar to Dolby Stereo, except louder. This matrix may introduce clipping when used on non-Dolby material. In general, when encoding to Dolby Stereo, the center and rear channels are made quieter. This version of dolby attempts to preserve the original volume. (The "dolby" matrix will not clip, but it makes other channels quieter as a consequence.)
- **qs**: Adhers to the "qs" matrix. Very similar to the "default" matrix, except that some minor widening is present. (See <https://en.wikipedia.org/wiki/QS_Regular_Matrix> for more information.)
- **rm**: Synonym for "qs". This option exists because it was common to mislabel qs-encoded recordings as rm.
- **sq**: Adheres to the "sq" matrix. Although this matrix had a lot of commercial releases in the late 1970s, its technical limitations held it back from widespread adoption. Due to SQ's flaws, this option should only be used on matrial explicitly encoded for SQ. (See <https://en.wikipedia.org/wiki/Stereo_Quadraphonic>).
- **sqloud**: Similar to SQ, except louder. This matrix may introduce clipping when used on non-SQ material. In general, when encoding to SQ, the rear channels are made quieter. This version of sq attempts to preserve the original volume. (The "sq" matrix will not clip, but it makes other channels quieter as a consequence.)

**-channels**: The channel layout in the output file

- **4**: Four-channel layout; quadraphonic. Includes front right and left; and rear front and left.
- **5**: Five-channel layout. Includes front right, center, and left; and rear front and left.
- **5.1**: Five-point-one channel layout. Includes front right, center, and left; rear front and left; and a subwoofer channel.

## Performance Options

**-low**: Specifies the lowest frequency calculated in the matrix. (Defaults to 20 hz.) Steering lower frequencies will make Soft Matrix run very slowly. If this is set too high, it may impede calculating the subwoofer or steering audible frequences. (Very low frequencies require a much larger window for fourier transforms. Larger windows take significantly longer to calculate.)

**-threads**: The number of threads to run. Defaults to [available_parallelism()](https://doc.rust-lang.org/stable/std/thread/fn.available_parallelism.html). This option is useful because available_parallelism() may return a number lower than the number of cores present in the CPU, and Soft Matrix currently does not adjust its number of threads while running. (<https://github.com/GWBasic/soft_matrix/issues/61>) Setting this higher than the number of cores in your CPU is not advised.

## Examples

### Upmix a wave file using all defaults

    soft_matrix stereo.wav surround.wav

This will upmix stereo.wav, the input file, to a 5.1 wav file named surround.wav, using the default matrix.

### Use the RM matrix

    soft_matrix stereo.wav surround.wav -matrix rm

This will upmix stereo.wav, the input file, to a 5.1 wav file named surround.wav, using the RM matrix.

### Only run a single thread

    soft_matrix stereo.wav surround.wav -threads 1

This will upmix stereo.wav, the input file, using only a single thread.

### Run a fast encode without an LFE channel

    soft_matrix stereo.wav preview.wav -low 60 -channels 5

This only steers frequencies above 60 hz. Useful for a quick preview of upmixing.

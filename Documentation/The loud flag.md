# The -loud Flag

When upmixing to 5 or 5.1 channels, there are some situations that could lead to [clipping](https://en.wikipedia.org/wiki/Clipping_(audio)) in the center channel and subwoofer. By default, Soft Matrix slightly lowers the volume to avoid clipping.

The -loud flag does not lower the volume; audio is played back at the same loudness as the source file.

## Background: Sounds played in both speakers (in stereo) are louder than when played in a single speaker

When playing back audio in stereo, imagine a sound playing as loud as the CD will allow, in the left speaker. Now, imagine that it's panned to the right speaker.

When the sound is in the middle, it is playing equally loud in each speaker. A curious thing *could* happen when panned through the middle, depending on how the CD was mixed:

- If the sound is as loud as the CD allows, it will sound slightly louder than when the sound was only playing in the left speaker.
- If the sound is 70.7% of the maximum loudness that a CD will allow, then it is equally as a loud as when the sound was in the left speaker.

## This can clip when deriving a center speaker

In Soft Matrix, to preserve the original volume of sounds steered to the center speaker, the following formula is used:

```
    center_speaker = (0.707106781186548 * left_speaker) + (0.707106781186548 * right_speaker)
```

Do you see the problem?

If the amplitude is greater than 1.0, [clipping](https://en.wikipedia.org/wiki/Clipping_(audio)) can occur.

For example, if the sound is as loud as possible (amplitude 1.0) in both speakers, the amplitude will be 1.41. This will result in audible distortion in the center speaker.

The same distortion could happen if the sound is a very low frequency steered to the subwoofer.

## How to avoid clipping

By default, Soft Matrix uses the -quiet option. This lowers the final output volume so that no clipping can happen.

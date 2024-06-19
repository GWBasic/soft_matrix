# How is Stereo Upmixed to Surround Sound?

This page explains how [Soft Matrix](/) upmixes stereo to surround sound.

[Soft Matrix](/) works with how your ear normally perceives two-speaker stereo to create a much more immersive experience in a surround sound environment.

It is a highly accurate [matrix decoder](https://en.wikipedia.org/wiki/Matrix_decoder).

## Intro: Fourier Transforms

Soft Matrix uses Fourier Transforms to inspect and steer audio.

If you don't know what a Fourier Transform is, I suggest reading: [An Interactive Introduction to Fourier Transforms](https://www.jezzamon.com/fourier/).

Soft Matrix works by calculating many Fourier Transforms over the entirety of a recording. When a Fourier Transform is calculated for both the right and left channels:

- The differences in [amplitude (loudness)](https://en.wikipedia.org/wiki/Amplitude) between the right and left channels is used to steer the sound right - to - left at playback.
- The differences in [phase](https://en.wikipedia.org/wiki/Phase_(waves)#Phase_shift) between the right and left channels is used to steer the sound front - to - back at playback.

## Sound Placement and Panning: Default Matrix

This section explains [Soft Matrix](/)'s default matrix. This matrix is suitable for recording that have significant out-of-phase material.

### Default Matrix: Right and Left Channels

Items hard panned to the right remain panned to the front right.

A sound that only comes from the right speaker will sound like it's coming from the front right corner of the room.
![Sound panned to the right](<How is Stereo Upmixed to Surround Sound/Stereo - Right.png>)

[Soft Matrix](/) preserves this panning when upmixed to stereo. The sound will continue to sound like it comes from the right front corner of the room.
![Sound panned to the front right](<How is Stereo Upmixed to Surround Sound/Surround - Right.png>)

The same happens with sounds hard-panned to the left. A sound that only comes from the left speaker will sound like it's coming from the front left corner of the room.

![Sound panned to the left](<How is Stereo Upmixed to Surround Sound/Stereo - Left.png>)

Likewise, the sound will continue to sound like it comes from the left front corner of the room.
![Sound panned to the front left](<How is Stereo Upmixed to Surround Sound/Surround - Left.png>)

### Default Matrix: Deriving a Center Speaker

[Soft Matrix](/) moves sounds that sound like they're coming from the center to the center speaker.

When listening to two speaker stereo, a sound that is played in equal volume in both speakers will sound like it comes from between the speakers:
![Sound panned to the center](<How is Stereo Upmixed to Surround Sound/Stereo - Front Center.png>)

In surround, the sound will come from the center speaker:
![Sound panned to the center speaker](<How is Stereo Upmixed to Surround Sound/Surround - Front Center.png>)

### Default Matrix: Deriving the Rear Speakers

When listening to two-speaker stereo, some sounds will "hang" in front of the speakers. This happens when sounds aren't completely in phase. [Soft Matrix](/) moves these sounds to the rear speakers.

In two speaker stereo, if the waveform is inverted, the sound will be diffuse and "hang" between the speakers:
![Out-of-phase sound](<How is Stereo Upmixed to Surround Sound/Stereo - Rear Center.png>)

[Soft Matrix](/) moves out-of-phase sounds to the rear:
![Out-of-phase sound is panned to the rear](<How is Stereo Upmixed to Surround Sound/Surround - Rear Center.png>)

It's also possible to hard pan to the rear right and rear left speakers. If the inverted waveform is very quiet, the sound will be mostly isolated to one of the rear speakers. (Note: It's impossible to 100% hard pan a rear speaker, otherwise the phase of the hiss would prevent playback of hard pans in the front speakers.)

For example, a sound played in the left speaker, with a very quiet inverted waveform in the right speaker, will generally sound like it's coming from the left corner of the room in stereo:
![A sound in the left speaker with a quiet inverted waveform in the right speaker](<How is Stereo Upmixed to Surround Sound/Stereo - Rear Side.png>)

[Soft Matrix](/) is able to place this sound in the rear left speaker:
![A sound in the left speaker with a quiet inverted waveform in the right speaker is played back in the left rear speaker](<How is Stereo Upmixed to Surround Sound/Surround - Rear Side.png>)

## Sound Placement and Panning: Horseshoe Matrix

Soft Matrix includes a horseshoe matrix. It is intended for material that is predominantly in-phase and that pans between the right and left speakers. The horseshoe matrix widens the stereo field and moves material that's hard panned to the right or left speaker towards the back.

### Horseshoe Matrix: Hard Pan to the Side

If a sound is hard panned to the left in stereo:
![Sound is hard panned to the left in stereo](<How is Stereo Upmixed to Surround Sound/Stereo - Left.png>)

It will be played between the left front and left rear speakers:
![Sound is panned between the left front and left rear speakers](<How is Stereo Upmixed to Surround Sound/Surround - Left Middle.png>)

The right side follows the same pattern.

### Horseshoe Matrix: Partial Pan to the Side

If a sound is partially panned to the left in stereo:
![Sound is partially panned to the left in stereo](<How is Stereo Upmixed to Surround Sound/Stereo - Left Center.png>)

Then the stereo field is widened and the sound panned more to the left:
![Stereo field widened and sound panned to the left](<How is Stereo Upmixed to Surround Sound/Surround - Left.png>)

The right side follows the same pattern.

### Horseshoe Matrix: Center and out-of-phase material

Just like the default matrix, material that is centered between the right and left speakers will play in the center speaker. Material that is out-of-phase will be isolated to the rear speakers.

## Dolby and QS / RM

The Dolby and QS (aka RM) matrixes are very similar to Soft Matrix's default matrix. There are slight adjustments to steering and levels based on publicly-available information.

See:

- [Dolby on Wikipedia](https://en.wikipedia.org/wiki/Dolby_Stereo#The_Dolby_Stereo_Matrix)
- [QS on Wikipedia](https://en.wikipedia.org/wiki/QS_Regular_Matrix)

## SQ

I (Andrew Rondeau, author) really struggled with SQ. I followed the specifications on [SQ's wikipedia page](https://en.wikipedia.org/wiki/Stereo_Quadraphonic) to encode test tones:

- It was very hard to come up with a deterministic algorithm that used both phase and right-left panning to calculate a panning location in the room.
- I could not keep the amplitude levels the same as my source material.

It's important to note that SQ has some fundamental flaws: When played back in stereo, material panned to the back is very loud in the left speaker. There are many common "quad" panning locations that cancel out in stereo or mono mixes. As a result, there are also multiple "SQ" matrixes used to encode. Due to my (Andrew Rondeau's) struggles with SQ, I didn't implement support for all of the different "SQ" matrixes.

## Credits

The following icons were used in generating the diagrams above:

- [Music by Flatart](https://thenounproject.com/icon/music-2594949/) from <a href="https://thenounproject.com/browse/icons/term/music/" target="_blank" title="Music Icons">Noun Project</a> (CC BY 3.0) ([Source SVG](<How is Stereo Upmixed to Surround Sound/Sources/noun-music-2594949.svg>))
- [Guitar by Flatart](https://thenounproject.com/icon/guitar-2594947/) from <a href="https://thenounproject.com/browse/icons/term/guitar/" target="_blank" title="Guitar Icons">Noun Project</a> (CC BY 3.0) ([Source SVG](<How is Stereo Upmixed to Surround Sound/Sources/noun-guitar-2594947.svg>))

These icons are released under the [CC BY 3.0 license](https://creativecommons.org/licenses/by/3.0/).

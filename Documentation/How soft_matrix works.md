# Overview

This page explains how [soft_matrix](/) upmixes stereo to surround sound.

[soft_matrix](/) works with how your ear normally perceives two-speaker stereo to create a much more immersive experience in a surround sound environment.

It is a highly accurate [matrix decoder](https://en.wikipedia.org/wiki/Matrix_decoder).

## Sound Placement and Panning: Default Matrix

This section explains [soft_matrix](/)'s default matrix. This matrix is suitable for recording that have significant out-of-phase material.

### Right and Left Channels

Items hard panned to the right remain panned to the front right.

A sound that only comes from the right speaker will sound like it's coming from the front right corner of the room.
![Sound panned to the right](<How it works/Stereo - Right.png>)

[soft_matrix](/) preserves this panning when upmixed to stereo. The sound will continue to sound like it comes from the right front corner of the room.
![Sound panned to the front right](<How it works/Surround - Right.png>)

The same happens with sounds hard-panned to the left. A sound that only comes from the left speaker will sound like it's coming from the front left corner of the room.

![Sound panned to the left](<How it works/Stereo - Left.png>)

Likewise, the sound will continue to sound like it comes from the left front corner of the room.
![Sound panned to the front left](<How it works/Surround - Left.png>)

### Deriving a Center Speaker

[soft_matrix](/) moves sounds that sound like they're coming from the center to the center speaker.

When listening to two speaker stereo, a sound that is played in equal volume in both speakers will sound like it comes from between the speakers:
![Sound panned to the center](<How it works/Stereo - Front Center.png>)

In surround, the sound will come from the center speaker:
![Sound panned to the center speaker](<How it works/Surround - Front Center.png>)

### Deriving the Rear Speakers

When listening to two-speaker stereo, some sounds will "hang" in front of the speakers. This happens when sounds aren't completely in phase. [soft_matrix](/) moves these sounds to the rear speakers.

In two speaker stereo, if the waveform is inverted, the sound will be diffuse and "hang" between the speakers:
![Out-of-phase sound](<How it works/Stereo - Rear Center.png>)

[soft_matrix](/) moves out-of-phase sounds to the rear:
![Out-of-phase sound is panned to the rear](<How it works/Surround - Rear Center.png>)

It's also possible to hard pan to the rear right and rear left speakers. If the inverted waveform is very quiet, the sound will be mostly isolated to one of the rear speakers. (Note: It's impossible to 100% hard pan a rear speaker, otherwise the phase of the hiss would prevent playback of hard pans in the front speakers.)

For example, a sound played in the left speaker, with a very quiet inverted waveform in the right speaker, will generally sound like it's coming from the left corner of the room in stereo:
![A sound in the left speaker with a quiet inverted waveform in the right speaker](<How it works/Stereo - Rear Side.png>)

[soft_matrix](/) is able to place this sound in the rear left speaker:
![A sound in the left speaker with a quiet inverted waveform in the right speaker is played back in the left rear speaker](<How it works/Surround - Rear Side.png>)

## Sound Placement and Panning: Horseshoe Matrix

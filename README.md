MIDI-to-Keypress
================

Takes MIDI events and turns them into keypresses.  This fork is modified for full typing based on [Anna Feit's paper](http://annafeit.de/pianotext/). 

Installing
----------

Download this [zip](https://github.com/xobs/midi-to-keypress/releases/latest), unzip it, and execute it from the terminal. 


Running
--------

This program requires Rust. Download it from [rustup.rs](https://rustup.rs).

To run, go into the directory where the repositry was downloaded and type:

````
cargo run
````

Make sure the midi device is connected before hand.

Usage
-----

To list available devices, run "miditran --list".  To specify a device to use as an input, run "miditran --device [device-name]".

Currently, there is no external configuration.  The program will search for a device named MIDI\_DEV\_NAME, and will monitor key events from that device.

For channel 0 (i.e. the main keys), it will translate keys into the following keyboard piano:

````
 h  g     z  k  y      w  u     n  c  l     g  v       q m u     []   2     5  7  9 
t  x  j  e  p  f  m  d  a  o  r  e  t  i  s  h  b  d  a  e  o  r  1  3  4  6  8  0
````

For keys one octave below C-4, it will additionally press the Ctrl key.  For keys one octave above C-4, it will instead press the Shift key.

For channel 9 (i.e. the drum pads above), pressing pads 1-4 will press Esc, followed by Ctrl+Alt+Shift+{Z, X, C, or V}.  This can be used to switch instruments.


For further documentation check [here](https://psyaito.github.io/blog/pianotype.html)

Credit
-----

1. xobs for making [midi-to-keypress](https://github.com/xobs/midi-to-keypress).
2. Anna Feit for the [research](http://annafeit.de/pianotext/).
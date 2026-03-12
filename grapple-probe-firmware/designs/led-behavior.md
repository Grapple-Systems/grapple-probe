# LED Behavior

The Grapple Probe has one tri-color led, but we can make it do lots of stuff to display state with blinking and mixing colors.

Things we want to communicate with the led.

- Level Shifter Voltage
- COM Port Activity
- Debug Port Activity
- Debug Port Connected

For level shifter voltage the green and blue leds are used.  When the level shifters are at 3.3V the led should be green, when 1.8V the led should be blue, in between the voltages the led should mix green and blue with a ratio relative to the voltage extents.  Green intensity is calculated by subtracting 1.8 from the level shifter voltage and dividing by 1.5.  Blue intensity is 1 minus the green intensity.  When the level shifters are off the led should be red.

When idle the led should be solid, upon a debug port connection the led should flash a few times and return to solid.  When there is debug port or uart activity the led should flash, but not toggle, such that the led always returns to solid.

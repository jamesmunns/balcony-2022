# Notes

So it takes about 25ms to update 100 LEDs, or about 39.5fps max.

If I make my phase accumulator a u16, and we aim for 32fps or so, that means a phase rate of 2048 would cause a full cycle in 1s. That gives us some time to set the min/max twinkle rate. e.g 1024 would be 2s, 4096 would be 0.5s.

 128   16s
 256    8s
 512    4s
1024    2s
2048    1s

I could probably just generate a random phase rate in the range of 128..=2048 and be good.

Something like `(rand_u16() >> 5).max(128))`.

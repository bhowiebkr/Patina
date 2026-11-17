; Paints random colors on the screen of the GPU, as the noise of an old TV. Each plot moves to the
; next pixel.
PLOT = 0xF3         ; ST plots the color A at the GPU X and Y, then X moves one pixel
RANDOM = 0xF5       ; LD gives a random byte
loop:   LD RANDOM
        ST PLOT
        JMP loop

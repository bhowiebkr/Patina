; Plots pixels of random colors at random places on the screen of the GPU. The GPU uses the low
; 5 bits of X and Y and the low 4 bits of the color.
GPU_X = 0xF1        ; the column of the next pixel
GPU_Y = 0xF2        ; the row of the next pixel
PLOT = 0xF3         ; ST plots the color A
RANDOM = 0xF5       ; LD gives a random byte
loop:   LD RANDOM
        ST GPU_X
        LD RANDOM
        ST GPU_Y
        LD RANDOM
        ST PLOT
        JMP loop

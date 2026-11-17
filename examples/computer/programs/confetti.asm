; Plots pixels of random colors at random places on the screen of the GPU. The GPU uses the low
; 5 bits of X and Y and the low 4 bits of the color.
loop:   LD 13
        ST 9        ; X = a random byte
        LD 13
        ST 10       ; Y = a random byte
        LD 13
        ST 11       ; plot a random color at X and Y
        JMP loop

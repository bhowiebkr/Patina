; Paints with the joystick. Click the pad of the joystick node, then hold the arrow keys. Each
; step plots a random color at the pen. The GPU uses the low 5 bits of X and Y, so the pen wraps.
        LDI 15
        ST 0        ; X = 15, the middle
        ST 1        ; Y = 15
loop:   LD 14       ; A = the x axis: 1 right, 255 left, else 0
        ADD 0
        ST 0        ; X = X + the x axis
        ST 9        ; the GPU X = X
        LD 15       ; A = the y axis: 1 down, 255 up, else 0
        ADD 1
        ST 1        ; Y = Y + the y axis
        ST 10       ; the GPU Y = Y
        LD 13       ; A = a random color
        ST 11       ; plot it at X and Y
        JMP loop

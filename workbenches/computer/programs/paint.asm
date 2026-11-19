; Paints with the joystick. Click the pad of the joystick node, then hold the arrow keys. Each
; step plots a random color at the pen. The GPU uses the low 5 bits of X and Y, so the pen wraps.
GPU_X = 0xF1        ; the column of the next pixel
GPU_Y = 0xF2        ; the row of the next pixel
PLOT = 0xF3         ; ST plots the color A
RANDOM = 0xF5       ; LD gives a random byte
JOY_X = 0xF6        ; the x axis: 1 right, -1 left, else 0
JOY_Y = 0xF7        ; the y axis: 1 down, -1 up, else 0
PEN_X = 0x00        ; the pen, in the RAM
PEN_Y = 0x01
        LDI 15
        ST PEN_X    ; the middle of the screen
        ST PEN_Y
loop:   LD JOY_X
        ADD PEN_X
        ST PEN_X    ; PEN_X = PEN_X + the x axis
        ST GPU_X
        LD JOY_Y
        ADD PEN_Y
        ST PEN_Y    ; PEN_Y = PEN_Y + the y axis
        ST GPU_Y
        LD RANDOM
        ST PLOT     ; plot a random color at the pen
        JMP loop

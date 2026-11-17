; Snake. Click the pad of the joystick node, then push the arrow keys to turn the snake. Eat the
; red food to grow. When the snake hits its body, the screen goes red and the game starts again.
; OUT gives the score. Each cell of the body keeps the direction to the next cell as its color,
; so the tail follows the head without a list of cells. A step is about 40 instructions, so set
; the clock to every = 1.
GPU_X = 0xF1        ; the column of the next pixel
GPU_Y = 0xF2        ; the row of the next pixel
PLOT = 0xF3         ; ST plots the color A, LD gives the color of the pixel
FILL = 0xF4         ; ST fills the screen with the color A
RANDOM = 0xF5       ; LD gives a random byte
BUTTONS = 0xF8      ; 1 up, 2 down, 4 left, 8 right
SPRITE_X = 0xFA     ; sprite 0 is the head
SPRITE_Y = 0xFB
SPRITE_COLOR = 0xFC
RIGHT = 2           ; the colors of the body: the direction to the next cell
DOWN = 3
LEFT = 10
UP = 11
FOOD = 12
HX = 0x10           ; the head
HY = 0x11           ; HX + 1, so LDX HX with X = 1 gives HY
TX = 0x12           ; the tail
TY = 0x13
DIR = 0x14          ; the direction of the head
DX = 0x15           ; the step of the head: 1, or 255 for -1
AX = 0x16           ; the axis of the head: 0 for X, 1 for Y
GROW = 0x17         ; the tail stops for this number of steps
SCORE = 0x18
STEP = 0x19         ; the step of the tail
NEW = 0x1A          ; the new direction
DELTA = 0x20        ; 0x20 + a direction: its step
AXIS = 0x30         ; 0x30 + a direction: its axis
KEYS = 0x40         ; 0x40 + the buttons: the direction, or 0
        LDI 1
        ST 0x22         ; RIGHT and DOWN step 1
        ST 0x23
        ST 0x33         ; DOWN and UP move on the Y axis
        ST 0x3B
        LDI 255
        ST 0x2A         ; LEFT and UP step -1
        ST 0x2B
        LDI UP
        ST 0x41         ; button 1
        LDI DOWN
        ST 0x42         ; button 2
        LDI LEFT
        ST 0x44         ; button 4
        LDI RIGHT
        ST 0x48         ; button 8
        LDI 14
        ST SPRITE_COLOR ; the head is yellow
start:  LDI 0
        ST FILL         ; clear the screen
        ST AX
        ST SCORE
        OUT
        LDI 1
        ST DX
        LDI 4
        ST GROW         ; the snake grows to 5 cells
        LDI 15
        ST HX           ; the snake starts in the middle
        ST HY
        ST TX
        ST TY
        ST GPU_X
        ST GPU_Y
        ST SPRITE_X
        ST SPRITE_Y
        LDI RIGHT
        ST DIR
        ST PLOT
        JMP place
loop:   LD BUTTONS
        TAX
        LDX KEYS        ; the direction of the buttons
        JZ tail
        ST NEW
        SUB DIR
        JZ tail         ; the same direction
        ANDI 15
        SUBI 8
        JZ tail         ; the snake cannot turn back
        LD NEW
        ST DIR
        TAX
        LDX DELTA
        ST DX
        LDX AXIS
        ST AX
        LD HX
        ST GPU_X
        LD HY
        ST GPU_Y
        LD DIR
        ST PLOT         ; the head cell gets the new direction
tail:   LD GROW
        JZ cut
        SUBI 1
        ST GROW         ; the tail stops, so the snake grows
        JMP head
cut:    LD TX
        ST GPU_X
        LD TY
        ST GPU_Y
        LD PLOT         ; the direction from the tail to the next cell
        TAX
        LDI 0
        ST PLOT         ; clear the tail cell
        LDX DELTA
        ST STEP
        LDX AXIS
        TAX
        LDX TX
        ADD STEP
        STX TX          ; the GPU uses the low 5 bits, so no AND is necessary
head:   LD AX
        TAX
        LDX HX
        ADD DX
        ANDI 31
        STX HX          ; the head wraps at the edges
        LD HX
        ST GPU_X
        ST SPRITE_X
        LD HY
        ST GPU_Y
        ST SPRITE_Y
        LD PLOT         ; the pixel in front of the head
        JZ free
        SUBI FOOD
        JZ eat
        LDI 4
        ST FILL         ; the snake hit its body
        LDI 40
wait:   SUBI 1
        JZ start
        JMP wait
eat:    LD GROW
        ADDI 3
        ST GROW
        LD SCORE
        ADDI 1
        ST SCORE
        OUT
        LD DIR
        ST PLOT
place:  LD RANDOM
        ST GPU_X
        LD RANDOM
        ST GPU_Y
        LD PLOT
        JZ put          ; try again until the pixel is free
        JMP place
put:    LDI FOOD
        ST PLOT
        JMP loop
free:   LD DIR
        ST PLOT
        JMP loop

; Shows a random byte on OUT at each key. Type a text into the keyboard and press Enter: each key
; gives a new number on the hex display and the LCD.
KEY = 0xF0          ; LD gives the next key of the keyboard
RANDOM = 0xF5       ; LD gives a random byte
wait:   LD KEY      ; A = the next key, or 0 when no key waits
        JZ wait
        LD RANDOM
        OUT
        JMP wait

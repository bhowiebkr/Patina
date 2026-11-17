; Shows a random byte on OUT at each key. Type a text into the keyboard and press Enter: each key
; gives a new number on the hex display and the LCD.
wait:   LD 8        ; A = the next key, or 0 when no key waits
        JZ wait
        LD 13       ; A = a random byte
        OUT
        JMP wait

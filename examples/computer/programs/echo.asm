; Shows each key of the keyboard on the terminal. Type a text into the keyboard and press Enter.
loop:   LD 8        ; A = the next key, or 0 when no key waits
        JZ loop
        ST 8        ; write A to the terminal
        JMP loop

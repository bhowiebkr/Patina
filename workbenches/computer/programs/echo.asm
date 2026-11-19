; Shows each key of the keyboard on the terminal. Type a text into the keyboard and press Enter.
KEY = 0xF0          ; LD gives the next key of the keyboard. ST writes to the terminal.
loop:   LD KEY      ; A = the next key, or 0 when no key waits
        JZ loop
        ST KEY      ; write A to the terminal
        JMP loop

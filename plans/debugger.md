enhance hackem to add source debugging

make hack_cc generate debug info in a separate file, you make up the format. this should include source line info, variable names, function names etc

then enhance hackem to support loading this data and using it to debug. The dissassembler window stays the same. Lets have a new debug window that should source lines, and allows the showing of the assembler too. This should work with .s files as weel - so i can debug the runtime or the user can write and debug their own .s routines.

I should be able to set break points at function names, and at filename:linenumber

add commands to display variable by name

other hackem fixes.

add a reset command the clears the screen and resets all registers

every time an address is displayed it should be in hex and decimal

add a view menu to allow opening and closing various windows, remove the dark, light, system buttons and make them view menu option.

add a run menu, make go and break entries there

the disassembler window should alays fill with code , scrolling and resising should correctly work. 


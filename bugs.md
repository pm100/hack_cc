broken things. and missing tests

all my tests done using hackem emulator in work\hackem

sizeof (variable) is not supported

char msg[4] = "abc"; 
int x = msg[0];
produces the wrong result (hex 10 when i tested it)

char mas[] = "abc";
does not compile

String literals were not being terminated with 0, i think i fixed it but please check

demo/cal.c fails in different ways dpeing on how app is compiled. fix this

All tests need to be executed using all formats and , if relevant, single line and multi step compile.
you need to add hackem format to the emulator in hack_cc. and add tst format support

add tests for the above described bugs and expliti check of null termination

#include <ctype.h>
#include <stdbool.h>
#include <stdio.h>

#include "libnyx.h"

#ifndef HEXDUMP_COLS
#define HEXDUMP_COLS 16
#endif
 
void hexdump(void *mem, unsigned int len)
{
        unsigned int i, j;
        
        for(i = 0; i < len + ((len % HEXDUMP_COLS) ? (HEXDUMP_COLS - len % HEXDUMP_COLS) : 0); i++)
        {
                /* print offset */
                if(i % HEXDUMP_COLS == 0)
                {
                        printf("0x%06x: ", i);
                }
 
                /* print hex data */
                if(i < len)
                {
                        printf("%02x ", 0xFF & ((char*)mem)[i]);
                }
                else /* end of block, just aligning for ASCII dump */
                {
                        printf("   ");
                }
                
                /* print ASCII dump */
                if(i % HEXDUMP_COLS == (HEXDUMP_COLS - 1))
                {
                        for(j = i - (HEXDUMP_COLS - 1); j <= i; j++)
                        {
                                if(j >= len) /* end of block, not really printing */
                                {
                                        putchar(' ');
                                }
                                else if(isprint(((char*)mem)[j])) /* printable char */
                                {
                                        putchar(0xFF & ((char*)mem)[j]);        
                                }
                                else /* other char */
                                {
                                        putchar('.');
                                }
                        }
                        putchar('\n');
                }
        }
}
 

int main(void){
  printf("YO\n");

  uint8_t payload_bytes[] = "HALLO";

  void* ptr = nyx_new("/tmp/nyx_bash/", "", 0, sizeof(payload_bytes), false);

  printf("QEMU Rust Object Pointer: %p\n", ptr);

  void* aux = nyx_get_aux_buffer(ptr);

  printf("QEMU Rust Aux Pointer: %p\n", aux);

  hexdump(aux, 16);

  void* payload_buf = nyx_get_input_buffer(ptr);

  nyx_set_afl_input(ptr, payload_bytes, sizeof(payload_bytes));


  printf("QEMU Rust Payload Pointer: %p\n", payload_buf);

  nyx_option_set_reload_mode(ptr, true);
  nyx_option_apply(ptr);

  hexdump(payload_buf, 16);

  printf("About to run init\n");
  printf("INIT -> %d\n", nyx_exec(ptr));
  printf("Init done\n");


  for(int i = 0; i < 32; i++){
        nyx_set_afl_input(ptr, payload_bytes, sizeof(payload_bytes));
        printf("nyx_exec -> %d\n", nyx_exec(ptr));
        //nyx_print_aux_buffer(ptr);
  }

  nyx_shutdown(ptr);


}

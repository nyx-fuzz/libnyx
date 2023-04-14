/* Simple test program to test the C-API */

#include <stdio.h>
#include "libnyx.h"

#include <stdio.h>
#include <ctype.h>

#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>
 
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
 
#define WORKDIR_PATH "/tmp/wdir"

int main(int argc, char** argv){

    void* aux_buffer; 


    void* nyx_config = nyx_config_load("/tmp/nyx_libxml2/");

    //nyx_config_debug(nyx_config);

    nyx_config_set_workdir_path(nyx_config, WORKDIR_PATH);
    nyx_config_set_input_buffer_size(nyx_config, 0x2000);

    int fd = open("/tmp/nyx_test_output.log", O_WRONLY | O_CREAT | O_TRUNC, 0644);
    printf("Log output FD: %d\n", fd);
    nyx_config_set_hprintf_fd(nyx_config, fd);

    nyx_config_set_process_role(nyx_config, StandAlone);
    
    //nyx_config_set_reuse_snapshot_path(nyx_config, "/tmp/wdir/snapshot/");

    nyx_config_print(nyx_config);
    nyx_config_debug(nyx_config);

    void* nyx_runner = nyx_new(nyx_config, 0);

    printf("Nyx runner object pointer: %p\n", nyx_runner);

    void* aux = nyx_get_aux_buffer(nyx_runner);

    printf("QEMU rust aux pointer: %p\n", aux);
    hexdump(aux, 16);

    void* nyx_input = nyx_get_input_buffer(nyx_runner);

    nyx_set_afl_input(nyx_runner, "INPUT", 5);
    printf("QEMU Rust Payload Pointer: %p\n", nyx_input);

    nyx_option_set_reload_mode(nyx_runner, true);
    nyx_option_apply(nyx_runner);

    hexdump(nyx_input, 16);

    printf("About to run init\n");
    printf("INIT -> %d\n", nyx_exec(nyx_runner));
    printf("Init done\n");

    for(int i = 0; i < 4; i++){
        nyx_set_afl_input(nyx_runner, "INPUT", 5);
        printf("nyx_exec -> %d\n", nyx_exec(nyx_runner));
        nyx_print_aux_buffer(nyx_runner);
    }

    nyx_shutdown(nyx_runner);

    if(!nyx_remove_work_dir(WORKDIR_PATH) ){
        printf("Error: Failed to remove work dir\n");
    }

}

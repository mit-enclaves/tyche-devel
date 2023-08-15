#include "tyche_api.h"
#include "sdk_tyche_rt.h"
#include "bricks.h"

// ————————————————————————————————— Hooks —————————————————————————————————— //
/// Entry point defined by the application.
extern void trusted_entry(frame_t* frame); 
// ——————————————————————————————— Functions ———————————————————————————————— //
void trusted_main(capa_index_t ret_handle, void *args)
{
  frame_t frame = {ret_handle, args}; 
 
  // Call the domain's main.
  trusted_entry(&frame); 

  // Done executing the domain, return.
  bricks_gate_call(&frame);
  // Should never return, if we do, an exit call happens.
}

// ———————————————————————————— Functions ported to Bricks ———————————————————————————— //

// extern int asm_call_gate(capa_index_t* capa, void** args);
// int gate_call(frame_t* frame)
// {
//   usize result = FAILURE;
//   result = asm_call_gate(&(frame->ret_handle), &(frame->args));
//   return result;
// }


// void* get_default_shared_buffer()
// {
//   return (void*) (shared_buffer);
// }

/// This is introduced by tychools.
// char *shared_buffer = (char*) 0x300000; 
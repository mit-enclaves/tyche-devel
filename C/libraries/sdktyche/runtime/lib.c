#include "tyche_api.h"
#include "sdk_tyche_rt.h"
// ———————————————————————————— Enclave sections ———————————————————————————— //


/// This is introduced by tychools.
char *shared_buffer = (char*) 0x300000; 

// ————————————————————————————————— Hooks —————————————————————————————————— //
/// Entry point defined by the application.
extern void trusted_entry(frame_t* frame); 

// ——————————————————————————————— Functions ———————————————————————————————— //
//
void trusted_main(capa_index_t ret_handle, void *args)
{
  frame_t frame = {ret_handle, args}; 
 
  // Call the domain's main.
  trusted_entry(&frame); 

  // Done executing the domain, return.
  gate_call(&frame);
  // Should never return, if we do, an exit call happens.
}

#if defined(CONFIG_X86)  || defined(__x86_64__)
extern int asm_call_gate(capa_index_t* capa, void** args);
#elif defined(CONFIG_RISCV) || defined(__riscv)
extern int riscv_asm_call_gate(capa_index_t* capa, void** args);
#endif

int gate_call(frame_t* frame)
{
  usize result = FAILURE;
  
#if defined(CONFIG_X86)  || defined(__x86_64__)
  result = asm_call_gate(&(frame->ret_handle), &(frame->args));
#elif defined(CONFIG_RISCV) || defined(__riscv)
  result = riscv_asm_call_gate(&(frame->ret_handle), &(frame->args));
#endif

  return result;
}

void* get_default_shared_buffer()
{
  return (void*) (shared_buffer);
}

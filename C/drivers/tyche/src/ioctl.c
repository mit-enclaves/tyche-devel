#include <linux/ioctl.h>
#include <linux/kernel.h>   /* printk() */
#include <linux/cdev.h> 
#include <linux/device.h>
#include <linux/fs.h>

#include "common.h"
#include "enclaves.h"
#define _IN_MODULE
#include "tyche_enclave.h"
#include "tyche_ioctl.h"
#undef _IN_MODULE
// —————————————————————— Global Driver Configuration ——————————————————————— //
static char* device_name = "tyche";
static char* device_class = "tyche";
static char* device_region = "tyche";

dev_t dev = 0;
static struct cdev tyche_cdev;
static struct class *dev_class;

// ———————————————————————————— File Operations ————————————————————————————— //

// File operation structure
static struct file_operations fops =
{
        .owner          = THIS_MODULE,
        .open           = tyche_open,
        .release        = tyche_close,
        .unlocked_ioctl = tyche_ioctl,
        .mmap           = tyche_mmap,
};

// ———————————————————————————— Driver Functions ———————————————————————————— //


int tyche_register(void)
{
  // Allocating Major number
  if((alloc_chrdev_region(&dev, 0, 1, device_region)) <0){
    ERROR("cannot allocate major number\n");
    return FAILURE;
  }
  LOG("Major = %d Minor = %d \n",MAJOR(dev), MINOR(dev));

  // Creating the cdev structure
  cdev_init(&tyche_cdev, &fops);

  // Adding character device to the system.
  if ((cdev_add(&tyche_cdev, dev, 1)) < 0)
  {
    ERROR("Cannot add the device to the system.\n");
    goto r_class;
  }

  // Creating the struct class.
  if ((dev_class = class_create(THIS_MODULE, device_class)) == NULL)
  {
    ERROR("Cannot create the struct class.\n");
    goto r_class;
  }

  // Creating the device.
  if ((device_create(dev_class, NULL, dev, NULL, device_name)) == NULL)
  {
    ERROR("Cannot create the Device 1\n");
    goto r_device;
  }

  init_enclaves();
  LOG("Tyche driver registered!\n");
  return SUCCESS; 

r_device:
  class_destroy(dev_class);
r_class:
  unregister_chrdev_region(dev, 1);
  return FAILURE;
}

void tyche_unregister(void)
{
  device_destroy(dev_class, dev);
  class_destroy(dev_class);
  cdev_del(&tyche_cdev);
  unregister_chrdev_region(dev, 1);
  LOG("Tyche driver unregistered!\n");
}

// —————————————————————————————————— API ——————————————————————————————————— //

int tyche_open(struct inode* inode, struct file* file) 
{
  if (file == NULL) {
    ERROR("We received a Null file descriptor.");
    goto failure;
  }
  if (create_enclave(file) != SUCCESS) {
    ERROR("Unable to create a new enclave");
    goto failure;
  }
  return SUCCESS;
failure:
  return FAILURE;
}

int tyche_close(struct inode* inode, struct file* handle)
{
   if (delete_enclave(handle) != SUCCESS) {
        ERROR("Unable to delete the enclave %p", handle);
        goto failure;
    }
  return SUCCESS;
failure:
  return FAILURE;
}


long tyche_ioctl(struct file* handle, unsigned int cmd, unsigned long arg)
{
  msg_enclave_info_t info = {UNINIT_USIZE, UNINIT_USIZE}; 
  msg_enclave_commit_t commit = {0, 0, 0};
  msg_enclave_mprotect_t mprotect = {0, 0, 0, 0};
  msg_enclave_switch_t transition = {0};
  switch(cmd) {
    case TYCHE_ENCLAVE_GET_PHYSOFFSET:
      if (get_physoffset_enclave(handle, &info.physoffset) != SUCCESS) {
        ERROR("Unable to get the physoffset for enclave %p", handle);
        goto failure;
      }
      if (copy_to_user(
            (msg_enclave_info_t*) arg, 
            &info, 
            sizeof(msg_enclave_info_t))) {
        ERROR("Unable to copy enclave physoffset for %p", handle);
        goto failure;
      }
      break;
    case TYCHE_ENCLAVE_COMMIT:
      if (copy_from_user(
            &commit,
            (msg_enclave_commit_t*) arg,
            sizeof(msg_enclave_commit_t))) {
        ERROR("Unable to copy commit arguments from user.");
        goto failure;
      }
      if (commit_enclave(
            handle,
            commit.page_tables,
            commit.entry,
            commit.stack) != SUCCESS) {
        ERROR("Commit failed for enclave %p", handle);
        goto failure;
      }
      break;
    case TYCHE_ENCLAVE_MPROTECT:
      if (copy_from_user(
            &mprotect,
            (msg_enclave_mprotect_t*) arg,
            sizeof(msg_enclave_mprotect_t))) {
        ERROR("Unable to copy arguments from user.");
        goto failure;
      }
      if (mprotect_enclave(
            handle,
            mprotect.start,
            mprotect.size,
            mprotect.flags,
            mprotect.tpe) != SUCCESS) {
        ERROR("Unable to mprotect he region for enclave %p", handle);
        goto failure;
      }
      break;
    case TYCHE_TRANSITION:
      if (copy_from_user(
            &transition,
            (msg_enclave_switch_t*) arg,
            sizeof(msg_enclave_switch_t))) {
        ERROR("Unable to copy arguments from user.");
        goto failure;
      }
      if (switch_enclave(handle, transition.args) != SUCCESS) {
        ERROR("Unable to switch to enclave %p", handle);
        goto failure;
      }
      break;
    default:
      ERROR("The command is not valid!");
      goto failure;
  }
  return SUCCESS;
failure:
  return FAILURE;
}

int tyche_mmap(struct file *file, struct vm_area_struct *vma)
{
  return mmap_segment(file, vma);
}

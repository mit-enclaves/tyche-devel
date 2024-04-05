#include <stdio.h>
#include "ssl.h"

// Generate with brssl chain from the redis.cert
const unsigned char CERT0[] = {
        0x30, 0x82, 0x03, 0x97, 0x30, 0x82, 0x02, 0x7F, 0xA0, 0x03, 0x02, 0x01,
        0x02, 0x02, 0x14, 0x07, 0xEA, 0xC7, 0x2F, 0xFE, 0x45, 0x33, 0xE7, 0x0F,
        0x71, 0xB1, 0x20, 0x70, 0x83, 0xD2, 0x72, 0xEC, 0x87, 0xB8, 0xFE, 0x30,
        0x0D, 0x06, 0x09, 0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x01, 0x0B,
        0x05, 0x00, 0x30, 0x5B, 0x31, 0x0B, 0x30, 0x09, 0x06, 0x03, 0x55, 0x04,
        0x06, 0x13, 0x02, 0x55, 0x4B, 0x31, 0x0A, 0x30, 0x08, 0x06, 0x03, 0x55,
        0x04, 0x08, 0x0C, 0x01, 0x4B, 0x31, 0x0A, 0x30, 0x08, 0x06, 0x03, 0x55,
        0x04, 0x07, 0x0C, 0x01, 0x4B, 0x31, 0x0A, 0x30, 0x08, 0x06, 0x03, 0x55,
        0x04, 0x0A, 0x0C, 0x01, 0x4B, 0x31, 0x0A, 0x30, 0x08, 0x06, 0x03, 0x55,
        0x04, 0x0B, 0x0C, 0x01, 0x4B, 0x31, 0x0A, 0x30, 0x08, 0x06, 0x03, 0x55,
        0x04, 0x03, 0x0C, 0x01, 0x4B, 0x31, 0x10, 0x30, 0x0E, 0x06, 0x09, 0x2A,
        0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x09, 0x01, 0x16, 0x01, 0x4B, 0x30,
        0x1E, 0x17, 0x0D, 0x32, 0x34, 0x30, 0x34, 0x30, 0x35, 0x31, 0x35, 0x33,
        0x36, 0x32, 0x39, 0x5A, 0x17, 0x0D, 0x32, 0x34, 0x30, 0x35, 0x30, 0x35,
        0x31, 0x35, 0x33, 0x36, 0x32, 0x39, 0x5A, 0x30, 0x5B, 0x31, 0x0B, 0x30,
        0x09, 0x06, 0x03, 0x55, 0x04, 0x06, 0x13, 0x02, 0x55, 0x4B, 0x31, 0x0A,
        0x30, 0x08, 0x06, 0x03, 0x55, 0x04, 0x08, 0x0C, 0x01, 0x4B, 0x31, 0x0A,
        0x30, 0x08, 0x06, 0x03, 0x55, 0x04, 0x07, 0x0C, 0x01, 0x4B, 0x31, 0x0A,
        0x30, 0x08, 0x06, 0x03, 0x55, 0x04, 0x0A, 0x0C, 0x01, 0x4B, 0x31, 0x0A,
        0x30, 0x08, 0x06, 0x03, 0x55, 0x04, 0x0B, 0x0C, 0x01, 0x4B, 0x31, 0x0A,
        0x30, 0x08, 0x06, 0x03, 0x55, 0x04, 0x03, 0x0C, 0x01, 0x4B, 0x31, 0x10,
        0x30, 0x0E, 0x06, 0x09, 0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x09,
        0x01, 0x16, 0x01, 0x4B, 0x30, 0x82, 0x01, 0x22, 0x30, 0x0D, 0x06, 0x09,
        0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x01, 0x01, 0x05, 0x00, 0x03,
        0x82, 0x01, 0x0F, 0x00, 0x30, 0x82, 0x01, 0x0A, 0x02, 0x82, 0x01, 0x01,
        0x00, 0x9D, 0x18, 0xE5, 0xDC, 0x74, 0x4F, 0x56, 0x17, 0x32, 0xFA, 0xC2,
        0x62, 0xEF, 0x9D, 0x29, 0xCD, 0x57, 0xC0, 0xE4, 0x5E, 0x6F, 0xAB, 0x5C,
        0xA0, 0xC5, 0xBB, 0x5A, 0xDE, 0xE6, 0x95, 0xE3, 0xB1, 0xF8, 0x74, 0x76,
        0x26, 0x6A, 0xF9, 0xA4, 0xD7, 0x2F, 0x5C, 0x16, 0xDF, 0xD4, 0x31, 0x4E,
        0x54, 0x46, 0xEB, 0x65, 0x2D, 0xC8, 0xBD, 0x8F, 0xDD, 0xF2, 0xD5, 0xAE,
        0x2B, 0x0B, 0x5F, 0xB4, 0xFE, 0x94, 0x0C, 0xB0, 0x35, 0x74, 0x56, 0x92,
        0x53, 0x45, 0xFE, 0x71, 0xFB, 0x3D, 0x93, 0xDC, 0xE1, 0x1B, 0x3F, 0xD4,
        0xAD, 0x93, 0x80, 0xF1, 0x1D, 0x6D, 0xA2, 0x56, 0xCB, 0x35, 0xC4, 0xAB,
        0x2D, 0x33, 0x15, 0xDC, 0x71, 0x6E, 0xEF, 0x38, 0x7E, 0xF7, 0x54, 0xA8,
        0x93, 0x83, 0xEE, 0x93, 0x6E, 0x75, 0xA5, 0xF9, 0xA4, 0xC1, 0x85, 0x8E,
        0x66, 0xA7, 0x3C, 0x81, 0x44, 0xE6, 0x35, 0x27, 0x04, 0x3E, 0x53, 0xE1,
        0x93, 0x4B, 0xF8, 0xB8, 0x82, 0x42, 0x6D, 0x5B, 0x57, 0x00, 0x96, 0x72,
        0x55, 0xEB, 0xB3, 0x76, 0xDF, 0x3D, 0x91, 0xAD, 0xF0, 0x02, 0x26, 0xC3,
        0x59, 0x35, 0xDC, 0x47, 0x48, 0xD5, 0x52, 0xED, 0x3F, 0x8F, 0xCB, 0xA1,
        0xCC, 0xB8, 0xDB, 0x2C, 0xFB, 0x09, 0xF3, 0xBF, 0xD0, 0xFF, 0x18, 0xB3,
        0x0D, 0x42, 0xAB, 0x6E, 0x70, 0xDE, 0x43, 0x1C, 0x4A, 0x2E, 0x6E, 0x87,
        0x8D, 0x1B, 0x1A, 0xA0, 0x72, 0x83, 0x56, 0x18, 0x0D, 0xCB, 0x16, 0x04,
        0xAA, 0xAE, 0x4D, 0x5F, 0x2C, 0xED, 0x93, 0xA6, 0xD0, 0xE2, 0x36, 0xC5,
        0xDE, 0x16, 0xC8, 0xF9, 0x1B, 0x84, 0xD3, 0xCE, 0x20, 0xF5, 0xED, 0xE8,
        0xA9, 0x53, 0x90, 0x84, 0xDC, 0xCF, 0x91, 0x90, 0x3B, 0x7F, 0x81, 0x6E,
        0xB9, 0xE9, 0x3B, 0x3E, 0x63, 0x5A, 0xFC, 0x07, 0x21, 0x99, 0x23, 0x59,
        0xAB, 0x8E, 0x84, 0x24, 0x07, 0x02, 0x03, 0x01, 0x00, 0x01, 0xA3, 0x53,
        0x30, 0x51, 0x30, 0x1D, 0x06, 0x03, 0x55, 0x1D, 0x0E, 0x04, 0x16, 0x04,
        0x14, 0x93, 0xDE, 0x87, 0xAB, 0xAA, 0x6E, 0x20, 0x1C, 0xAF, 0x82, 0xDE,
        0x35, 0xED, 0x97, 0xEF, 0xB7, 0x46, 0xDE, 0x2C, 0x8B, 0x30, 0x1F, 0x06,
        0x03, 0x55, 0x1D, 0x23, 0x04, 0x18, 0x30, 0x16, 0x80, 0x14, 0x93, 0xDE,
        0x87, 0xAB, 0xAA, 0x6E, 0x20, 0x1C, 0xAF, 0x82, 0xDE, 0x35, 0xED, 0x97,
        0xEF, 0xB7, 0x46, 0xDE, 0x2C, 0x8B, 0x30, 0x0F, 0x06, 0x03, 0x55, 0x1D,
        0x13, 0x01, 0x01, 0xFF, 0x04, 0x05, 0x30, 0x03, 0x01, 0x01, 0xFF, 0x30,
        0x0D, 0x06, 0x09, 0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x01, 0x0B,
        0x05, 0x00, 0x03, 0x82, 0x01, 0x01, 0x00, 0x0F, 0x67, 0xB2, 0x2E, 0xF7,
        0xE4, 0x10, 0xFA, 0x1B, 0x01, 0xAC, 0x5B, 0xEA, 0x72, 0xF3, 0x50, 0x2A,
        0xB8, 0xBA, 0x15, 0xB2, 0xB2, 0x61, 0x1A, 0xF5, 0xA6, 0xC7, 0xBE, 0xD1,
        0xC4, 0x24, 0x20, 0x8D, 0x4A, 0x0B, 0x92, 0x48, 0x9F, 0x24, 0x95, 0xA5,
        0xE6, 0x3C, 0xC7, 0x86, 0xB8, 0x7C, 0x46, 0xA0, 0xE8, 0xC0, 0xBB, 0x53,
        0x11, 0xC5, 0xFA, 0xE8, 0x55, 0xC0, 0xC7, 0x29, 0x82, 0xD5, 0xA3, 0x69,
        0x48, 0x6F, 0x98, 0x51, 0xBF, 0x89, 0x55, 0x74, 0x7B, 0x5C, 0x93, 0x0F,
        0xD8, 0xE1, 0x11, 0x54, 0x7B, 0xA3, 0x13, 0x66, 0xBB, 0x4A, 0x89, 0x10,
        0xFB, 0x19, 0x4B, 0xA6, 0x3E, 0x0D, 0x21, 0xF4, 0x1C, 0x92, 0x31, 0x13,
        0x8C, 0xCE, 0xB5, 0xDF, 0x3C, 0xD3, 0x56, 0xC1, 0x85, 0x0F, 0xF2, 0x23,
        0x37, 0x85, 0x5F, 0xF9, 0xF5, 0x70, 0xA0, 0x46, 0xF9, 0x99, 0xEA, 0xDA,
        0x2B, 0xBA, 0x93, 0x48, 0x51, 0xEC, 0xE1, 0xBF, 0x4C, 0x22, 0x28, 0x4B,
        0x12, 0x79, 0xAB, 0xBE, 0xD8, 0xAF, 0x2D, 0xCB, 0x67, 0x2E, 0xED, 0x79,
        0xB7, 0xD3, 0xD8, 0xE8, 0x2C, 0x53, 0xE1, 0xAF, 0xCD, 0x3B, 0x02, 0xB3,
        0x49, 0x37, 0x28, 0x00, 0x4A, 0x39, 0xEB, 0x7D, 0xA8, 0x2E, 0x25, 0x32,
        0x77, 0xDE, 0xB7, 0xD5, 0x4C, 0x5A, 0x06, 0x97, 0x8C, 0x9F, 0xD3, 0x7E,
        0x14, 0x70, 0x82, 0x9E, 0x41, 0x7E, 0x59, 0x97, 0xE7, 0x14, 0x97, 0x17,
        0x8D, 0x60, 0xE5, 0xE9, 0x79, 0x47, 0xCB, 0x5E, 0x85, 0x0B, 0x84, 0x81,
        0xC7, 0x0F, 0x99, 0xB7, 0x77, 0x61, 0x95, 0x31, 0x0E, 0x65, 0x2A, 0x6F,
        0x48, 0x3A, 0x12, 0x61, 0x58, 0x1C, 0x6F, 0xCC, 0xBB, 0xFE, 0x78, 0x29,
        0x66, 0xD8, 0xFF, 0xF9, 0x2B, 0xB1, 0xC0, 0xF2, 0xD0, 0x6B, 0x70, 0x82,
        0xA5, 0x8B, 0xF8, 0x78, 0x16, 0x2E, 0x1A, 0x27, 0x78, 0x66, 0x4A
};

const br_x509_certificate CHAIN[] = {
        { (unsigned char *)CERT0, sizeof CERT0 }
};

static const unsigned char RSA_P[] = {
        0xD7, 0x74, 0xEE, 0xEF, 0xEA, 0x07, 0x7A, 0x51, 0xD1, 0x22, 0x47, 0x2A,
        0x20, 0xF5, 0x95, 0x18, 0x22, 0xE2, 0x8B, 0x91, 0xB6, 0x25, 0x16, 0xC2,
        0xF0, 0xB5, 0x2E, 0xF2, 0x0D, 0x76, 0x1C, 0x00, 0xC2, 0xAE, 0x93, 0x1F,
        0x42, 0x08, 0x03, 0xEC, 0x8F, 0x75, 0x92, 0xAA, 0x69, 0x46, 0x33, 0x1E,
        0x33, 0x14, 0x01, 0x99, 0xBD, 0x65, 0x2B, 0x36, 0x0E, 0x66, 0x28, 0x14,
        0xFF, 0xF2, 0xDA, 0x31, 0xC8, 0xCD, 0xE6, 0x44, 0x8D, 0xF9, 0x3B, 0xE1,
        0xBD, 0xB7, 0x19, 0xE5, 0xF9, 0xCD, 0x83, 0x3F, 0x06, 0x02, 0xDD, 0xDF,
        0x1D, 0x6E, 0x7C, 0x66, 0xC5, 0x4C, 0x9E, 0xE0, 0x6A, 0x5F, 0xA2, 0x76,
        0x46, 0xF1, 0x15, 0xD4, 0xBD, 0xD1, 0xF8, 0x8F, 0x08, 0xBB, 0xE2, 0xFA,
        0x0F, 0xDF, 0x13, 0x4F, 0xAC, 0x7A, 0xD7, 0x32, 0x8A, 0x4F, 0x41, 0x1D,
        0x15, 0xF0, 0x59, 0x96, 0xB5, 0x3D, 0x90, 0x29
};

static const unsigned char RSA_Q[] = {
        0xBA, 0xA8, 0xA5, 0xC6, 0x64, 0x43, 0xF9, 0x7C, 0x5D, 0x3A, 0xD9, 0xB9,
        0x13, 0xA2, 0x30, 0xC8, 0x5D, 0xB3, 0x99, 0xD1, 0x6B, 0xFE, 0xC0, 0x0E,
        0xB6, 0xA4, 0x1B, 0xD3, 0x64, 0x09, 0x6B, 0x83, 0x80, 0x13, 0xFF, 0x33,
        0xC4, 0x0B, 0xD7, 0x60, 0xC9, 0x7F, 0xEB, 0x5D, 0x37, 0x05, 0x3F, 0xCC,
        0xB3, 0x6F, 0x8A, 0xA6, 0xF3, 0x87, 0xC3, 0x0C, 0x06, 0x06, 0x86, 0xCB,
        0x9E, 0x45, 0x7C, 0x2C, 0x7C, 0xAF, 0x09, 0x2F, 0xE7, 0xEF, 0x02, 0x2E,
        0xF0, 0x47, 0x88, 0xAC, 0xBD, 0x48, 0x76, 0x3A, 0xB2, 0x7A, 0xDB, 0x0D,
        0xED, 0xBE, 0x7A, 0x03, 0xC9, 0x78, 0x65, 0x8A, 0x75, 0x89, 0x34, 0xE2,
        0x20, 0xB3, 0x9C, 0x7D, 0xE9, 0x67, 0x7B, 0x71, 0x8E, 0x3C, 0x8E, 0x8C,
        0xAF, 0x84, 0xBC, 0x60, 0x76, 0x63, 0xBA, 0x8F, 0x89, 0x10, 0x8E, 0x88,
        0x1C, 0x72, 0x4D, 0x24, 0x82, 0xEC, 0xD8, 0xAF
};

static const unsigned char RSA_DP[] = {
        0x5C, 0xAF, 0xA8, 0x0E, 0x3B, 0x6E, 0x26, 0x17, 0xC6, 0x50, 0xE9, 0xAE,
        0x5C, 0xE9, 0x68, 0xCF, 0x2E, 0x4A, 0xA8, 0xE1, 0xF1, 0x2A, 0x79, 0x65,
        0x39, 0x29, 0xA8, 0x5D, 0x66, 0x9F, 0x15, 0xA2, 0xDA, 0x1D, 0x41, 0x9B,
        0x23, 0xCB, 0xD0, 0xEC, 0x56, 0x36, 0xAC, 0xF6, 0x74, 0x3D, 0x47, 0xC6,
        0x49, 0x10, 0xE5, 0x33, 0x5E, 0xFF, 0x83, 0x9C, 0x48, 0x8B, 0x77, 0xD8,
        0xB8, 0xD6, 0x9F, 0x38, 0xE5, 0x7A, 0x76, 0x01, 0xAD, 0xD5, 0xB7, 0x06,
        0x00, 0x98, 0x21, 0x23, 0x06, 0xD8, 0x7B, 0x0A, 0x84, 0xAA, 0x7D, 0x09,
        0xFB, 0x5E, 0x49, 0x53, 0xE8, 0xB2, 0x72, 0x72, 0x76, 0x30, 0x57, 0xF2,
        0x6B, 0xC8, 0x50, 0xAC, 0xE9, 0x4F, 0xC7, 0x8E, 0xB8, 0xA2, 0x23, 0x1D,
        0x91, 0xF0, 0x54, 0x1D, 0x65, 0x44, 0x9F, 0x08, 0xD5, 0xE9, 0x0C, 0x48,
        0xD7, 0xE4, 0x42, 0x96, 0x0B, 0xB7, 0xC5, 0x29
};

static const unsigned char RSA_DQ[] = {
        0x1C, 0x48, 0x35, 0x66, 0x0C, 0x07, 0x28, 0xA4, 0x29, 0x54, 0x23, 0x6D,
        0x21, 0x86, 0x6F, 0xB1, 0xCC, 0x50, 0xCC, 0x3B, 0xA9, 0x0B, 0x5E, 0x7A,
        0x5C, 0x3E, 0x1D, 0x61, 0x38, 0x45, 0x1D, 0x1F, 0x3D, 0xA6, 0xCA, 0x02,
        0x43, 0xF0, 0x2F, 0x60, 0x20, 0xE7, 0xDA, 0xF7, 0xB2, 0xC0, 0x7E, 0xDC,
        0x3B, 0x4B, 0xE9, 0x4C, 0x46, 0x96, 0x09, 0x7D, 0xA6, 0xE4, 0x12, 0x44,
        0x83, 0xE4, 0xAF, 0x5D, 0xE6, 0x3E, 0x77, 0x3B, 0xE4, 0xFE, 0x97, 0xEC,
        0x18, 0xC8, 0x1D, 0xF3, 0x5E, 0x72, 0xBE, 0x47, 0x42, 0x87, 0xCE, 0xED,
        0x1B, 0x5A, 0xC3, 0x0E, 0x13, 0xD6, 0xC4, 0x3B, 0xE8, 0x77, 0x33, 0xA6,
        0x17, 0xA2, 0x5F, 0x51, 0xCC, 0xAD, 0xBB, 0x4C, 0x87, 0x6A, 0xB1, 0x86,
        0xAB, 0x89, 0x87, 0x29, 0x6E, 0x86, 0xC9, 0xDB, 0xB9, 0xBE, 0xE2, 0x79,
        0xC9, 0x25, 0xA0, 0x7E, 0xA9, 0xBF, 0xCD, 0x4D
};

static const unsigned char RSA_IQ[] = {
        0x63, 0xF5, 0x06, 0xBB, 0x91, 0x5A, 0x1D, 0xE3, 0xA6, 0x02, 0x01, 0x71,
        0x4B, 0x33, 0xAF, 0x73, 0x68, 0x29, 0xB3, 0x0C, 0xD0, 0xEE, 0xBD, 0x85,
        0x81, 0x56, 0x95, 0xD2, 0xC2, 0x4A, 0xA5, 0xFE, 0x9E, 0xD4, 0x21, 0xF8,
        0x21, 0x06, 0xFF, 0x47, 0x06, 0x84, 0xF0, 0xE2, 0x43, 0xF8, 0x84, 0x77,
        0xB7, 0x95, 0x54, 0x77, 0x24, 0xB3, 0x7F, 0x42, 0x33, 0xE6, 0x6C, 0xC4,
        0xB3, 0xD6, 0x7A, 0xE6, 0xBF, 0xAC, 0x4E, 0x51, 0x80, 0x50, 0x9B, 0x5F,
        0x48, 0x7C, 0x53, 0xED, 0x57, 0x0B, 0xEB, 0x7D, 0xCC, 0xE3, 0x05, 0x80,
        0x30, 0x83, 0x49, 0x98, 0x7F, 0x68, 0x7E, 0x9D, 0x89, 0xAF, 0x69, 0x8C,
        0x2E, 0x03, 0xED, 0x7D, 0x80, 0x6D, 0xE9, 0xC6, 0xAD, 0x60, 0x37, 0x30,
        0x08, 0x20, 0x50, 0x06, 0xD5, 0xDE, 0x90, 0x65, 0x24, 0x52, 0x35, 0xF5,
        0xFD, 0xB9, 0xF1, 0xF2, 0x70, 0xFD, 0xB1, 0xA1
};

const br_rsa_private_key RSA = {
        2048,
        (unsigned char *)RSA_P, sizeof RSA_P,
        (unsigned char *)RSA_Q, sizeof RSA_Q,
        (unsigned char *)RSA_DP, sizeof RSA_DP,
        (unsigned char *)RSA_DQ, sizeof RSA_DQ,
        (unsigned char *)RSA_IQ, sizeof RSA_IQ
};


static int
get_cert_signer_algo(const br_x509_certificate *xc)
{
	br_x509_decoder_context dc;
	int err;

	br_x509_decoder_init(&dc, 0, 0);
	br_x509_decoder_push(&dc, xc->data, xc->data_len);
	err = br_x509_decoder_last_error(&dc);
	if (err != 0) {
		fprintf(stderr,
			"ERROR: certificate decoding failed with error %d\n",
			-err);
		return 0;
	}
	return br_x509_decoder_get_signer_key_type(&dc);
}

void
custom_server_profile(br_ssl_server_context *cc,
	const br_x509_certificate *chain, size_t chain_len,
	const br_rsa_private_key *sk)
{
	static const uint16_t suites[] = {
		BR_TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256,
		BR_TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384,
		BR_TLS_ECDHE_RSA_WITH_AES_128_CBC_SHA256,
		BR_TLS_ECDHE_RSA_WITH_AES_256_CBC_SHA384,
		BR_TLS_ECDHE_RSA_WITH_AES_128_CBC_SHA,
		BR_TLS_ECDHE_RSA_WITH_AES_256_CBC_SHA,
		BR_TLS_RSA_WITH_AES_128_GCM_SHA256,
		BR_TLS_RSA_WITH_AES_256_GCM_SHA384,
		BR_TLS_RSA_WITH_AES_128_CBC_SHA256,
		BR_TLS_RSA_WITH_AES_256_CBC_SHA256,
		BR_TLS_RSA_WITH_AES_128_CBC_SHA,
		BR_TLS_RSA_WITH_AES_256_CBC_SHA,
		BR_TLS_ECDHE_RSA_WITH_3DES_EDE_CBC_SHA,
		BR_TLS_RSA_WITH_3DES_EDE_CBC_SHA
	};

	br_ssl_server_zero(cc);
	br_ssl_engine_set_versions(&cc->eng, BR_TLS10, BR_TLS12);

	br_ssl_engine_set_prf10(&cc->eng, &br_tls10_prf);
	br_ssl_engine_set_prf_sha256(&cc->eng, &br_tls12_sha256_prf);
	br_ssl_engine_set_prf_sha384(&cc->eng, &br_tls12_sha384_prf);

	/*
	 * Apart from the requirements listed in the client side, these
	 * hash functions are also used by the server to compute its
	 * signature on ECDHE parameters. Which functions are needed
	 * depends on what the client may support; furthermore, the
	 * client may fail to send the relevant extension, in which
	 * case the server will default to whatever it can (as per the
	 * standard, it should be SHA-1 in that case).
	 */
	br_ssl_engine_set_hash(&cc->eng, br_md5_ID, &br_md5_vtable);
	br_ssl_engine_set_hash(&cc->eng, br_sha1_ID, &br_sha1_vtable);
	br_ssl_engine_set_hash(&cc->eng, br_sha224_ID, &br_sha224_vtable);
	br_ssl_engine_set_hash(&cc->eng, br_sha256_ID, &br_sha256_vtable);
	br_ssl_engine_set_hash(&cc->eng, br_sha384_ID, &br_sha384_vtable);
	br_ssl_engine_set_hash(&cc->eng, br_sha512_ID, &br_sha512_vtable);

	br_ssl_engine_set_suites(&cc->eng, suites,
		(sizeof suites) / (sizeof suites[0]));

	/*
	 * Elliptic curve implementation is used for ECDHE suites (but
	 * not for ECDH).
	 */
	br_ssl_engine_set_ec(&cc->eng, &br_ec_prime_i31);

	/*
	 * Set the "server policy": handler for the certificate chain
	 * and private key operations. Here, we indicate that the RSA
	 * private key is fit for both signing and decrypting, and we
	 * provide the two relevant implementations.

	 * BR_KEYTYPE_KEYX allows TLS_RSA_*, BR_KEYTYPE_SIGN allows
	 * TLS_ECDHE_RSA_*.
	 */
		br_ssl_server_set_single_rsa(cc, chain, chain_len, sk,
		BR_KEYTYPE_KEYX | BR_KEYTYPE_SIGN,
		br_rsa_i31_private, br_rsa_i31_pkcs1_sign);
	/*
	 * If the server used an EC private key, this call would look
	 * like this:
	int cert_signer_algo = get_cert_signer_algo(chain);
	br_ssl_server_set_single_ec(cc, chain, chain_len, sk,
		BR_KEYTYPE_KEYX | BR_KEYTYPE_SIGN,
		cert_signer_algo,
		&br_ec_prime_i31, br_ecdsa_i31_sign_asn1);

	 * Note the tricky points:
	 *
	 * -- "ECDH" cipher suites use only the EC code (&br_ec_prime_i31);
	 *    the ECDHE_ECDSA cipher suites need both the EC code and
	 *    the ECDSA signature implementation.
	 *
	 * -- For "ECDH" (not "ECDHE") cipher suites, the engine must
	 *    know the key type (RSA or EC) for the intermediate CA that
	 *    issued the server's certificate; this is an artefact of
	 *    how the protocol is defined. BearSSL won't try to decode
	 *    the server's certificate to obtain that information (it
	 *    could do that, the code is there, but it would increase the
	 *    footprint). So this must be provided by the caller.
	 *
	 * -- BR_KEYTYPE_KEYX allows ECDH, BR_KEYTYPE_SIGN allows
	 *    ECDHE_ECDSA.
	 */

	br_ssl_engine_set_cbc(&cc->eng,
		&br_sslrec_in_cbc_vtable,
		&br_sslrec_out_cbc_vtable);
	br_ssl_engine_set_gcm(&cc->eng,
		&br_sslrec_in_gcm_vtable,
		&br_sslrec_out_gcm_vtable);

	br_ssl_engine_set_aes_cbc(&cc->eng,
		&br_aes_ct_cbcenc_vtable,
		&br_aes_ct_cbcdec_vtable);
	br_ssl_engine_set_aes_ctr(&cc->eng,
		&br_aes_ct_ctr_vtable);
	/* Alternate: aes_ct64
	br_ssl_engine_set_aes_cbc(&cc->eng,
		&br_aes_ct64_cbcenc_vtable,
		&br_aes_ct64_cbcdec_vtable);
	br_ssl_engine_set_aes_ctr(&cc->eng,
		&br_aes_ct64_ctr_vtable);
	*/
	/* Alternate: aes_small
	br_ssl_engine_set_aes_cbc(&cc->eng,
		&br_aes_small_cbcenc_vtable,
		&br_aes_small_cbcdec_vtable);
	br_ssl_engine_set_aes_ctr(&cc->eng,
		&br_aes_small_ctr_vtable);
	*/
	/* Alternate: aes_big
	br_ssl_engine_set_aes_cbc(&cc->eng,
		&br_aes_big_cbcenc_vtable,
		&br_aes_big_cbcdec_vtable);
	br_ssl_engine_set_aes_ctr(&cc->eng,
		&br_aes_big_ctr_vtable);
	*/
	br_ssl_engine_set_des_cbc(&cc->eng,
		&br_des_ct_cbcenc_vtable,
		&br_des_ct_cbcdec_vtable);
	/* Alternate: des_tab
	br_ssl_engine_set_des_cbc(&cc->eng,
		&br_des_tab_cbcenc_vtable,
		&br_des_tab_cbcdec_vtable);
	*/

	br_ssl_engine_set_ghash(&cc->eng,
		&br_ghash_ctmul);
	/* Alternate: ghash_ctmul32
	br_ssl_engine_set_ghash(&cc->eng,
		&br_ghash_ctmul32);
	*/
	/* Alternate: ghash_ctmul64
	br_ssl_engine_set_ghash(&cc->eng,
		&br_ghash_ctmul64);
	*/
}

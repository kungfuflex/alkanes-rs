// Memory copy utility
export function memcpy(dest: usize, src: usize, len: usize): usize {
  memory.copy(dest, src, len);
  return dest;
}

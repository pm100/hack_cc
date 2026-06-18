#ifndef __STDARG_H__
#define __STDARG_H__

typedef char *va_list;

char *__va_arg(char **ap, int size);

#define va_start(ap, last) ((ap) = (va_list)(&(last) + 1))
#define va_arg(ap, type) (*(type *)__va_arg(&(ap), sizeof(type)))
#define va_end(ap) ((ap) = (va_list)0)
#define va_copy(dst, src) ((dst) = (src))

#endif /* __STDARG_H__ */

/* Strict bounded manifest/capture framing. No librime dependency. */
#ifndef XHUP_AUDIT_INPUT_H
#define XHUP_AUDIT_INPUT_H
#include <stdio.h>
#include <string.h>
#define AUDIT_ROW_BYTES 65536

/* 1=data, 0=EOF, -1=malformed/I/O error. Blank/comment lines are only allowed
 * in a source manifest, never in a captured menu correspondence. */
static int audit_row(FILE *file, char *line, int comments) {
  while (fgets(line, AUDIT_ROW_BYTES, file)) {
    size_t n = strlen(line);
    if (!n || line[n-1] != '\n') return -1; /* includes truncation/embedded NUL */
    while (n && (line[n-1]=='\n' || line[n-1]=='\r')) line[--n]=0;
    if (comments && (!n || line[0]=='#')) continue;
    char *tab = strchr(line,'\t');
    if (!tab || tab==line || !tab[1] || strchr(tab+1,'\t')) return -1;
    if (strspn(line,"abcdefghijklmnopqrstuvwxyz") != (size_t)(tab-line)) return -1;
    return 1;
  }
  return ferror(file) ? -1 : 0;
}

static int audit_inputs(const char *manifest, const char *capture) {
  FILE *source=fopen(manifest,"rb"), *saved=capture?fopen(capture,"rb"):NULL;
  if (!source || (capture && !saved)) {
    if (source) fclose(source);
    if (saved) fclose(saved);
    return 0;
  }
  char a[AUDIT_ROW_BYTES], b[AUDIT_ROW_BYTES];
  long count=0; int state, valid=1;
  while ((state=audit_row(source,a,1))==1) {
    ++count;
    if (saved) {
      if (audit_row(saved,b,0)!=1) {valid=0;break;}
      size_t key_length=(size_t)(strchr(a,'\t')-a);
      if (strncmp(a,b,key_length+1)) {valid=0;break;}
    }
  }
  if (state<0 || !count || (saved && audit_row(saved,b,0)!=0)) valid=0;
  if (fclose(source)) valid=0;
  if (saved && fclose(saved)) valid=0;
  return valid;
}

static int audit_status(int result, int reported_failures) {
  return result ? result : reported_failures ? 1 : 0;
}
#endif

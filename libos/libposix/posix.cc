// -*- mode: c++; c-file-style: "k&r"; c-basic-offset: 4 -*-
/***********************************************************************
 *
 * libos/libposix/posix.cc
 *   POSIX implementation of libos interface
 *
 * Copyright 2018 Irene Zhang  <irene.zhang@microsoft.com>
 *
 * Permission is hereby granted, free of charge, to any person
 * obtaining a copy of this software and associated documentation
 * files (the "Software"), to deal in the Software without
 * restriction, including without limitation the rights to use, copy,
 * modify, merge, publish, distribute, sublicense, and/or sell copies
 * of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be
 * included in all copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
 * EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
 * MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
 * NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS
 * BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN
 * ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN
 * CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 *
 **********************************************************************/

#include "common/library.h"
#include "include/io-queue.h"
#include "posix-queue.h"

namespace Zeus {
static QueueLibrary<POSIX::PosixQueue> lib;
    
int queue(int domain, int type, int protocol)
{
    return lib.queue(domain, type, protocol);
}

int bind(int qd, struct sockaddr *saddr, socklen_t size)
{
    return lib.bind(qd, saddr, size);
}

int accept(int qd, struct sockaddr *saddr, socklen_t *size)
{
    return lib.accept(qd, saddr, size);
}

int listen(int qd, int backlog)
{
    return lib.listen(qd, backlog);
}
        

int connect(int qd, struct sockaddr *saddr, socklen_t size)
{
    return lib.connect(qd, saddr, size);
}

int open(const char *pathname, int flags)
{
    return lib.open(pathname, flags);
}

int open(const char *pathname, int flags, mode_t mode)
{
    return lib.open(pathname, flags, mode);
}

int creat(const char *pathname, mode_t mode)
{
    return lib.creat(pathname, mode);
}
    
int close(int qd)
{
    return lib.close(qd);
}

int qd2fd(int qd)
{
    return lib.qd2fd(qd);
}
    
qtoken push(int qd, struct Zeus::sgarray &sga)
{
    return lib.push(qd, sga);
}

qtoken pop(int qd, struct Zeus::sgarray &sga)
{
    return lib.pop(qd, sga);
}

ssize_t wait_any(qtoken qts[], size_t num_qts)
{
    return lib.wait_any(qts, num_qts);
}

ssize_t wait_all(qtoken qts[], size_t num_qts)
{
    return lib.wait_all(qts, num_qts);
}

int merge(int qd1, int qd2)
{
    return lib.merge(qd1, qd2);
}

int filter(int qd, bool (*filter)(struct sgarray &sga))
{
    return lib.filter(qd, filter);
}

} // namespace Zeus
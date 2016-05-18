# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#    http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or
# implied.
# See the License for the specific language governing permissions and
# limitations under the License.

from __future__ import absolute_import, division, print_function

from cryptography import utils
from cryptography.hazmat.primitives.asymmetric import dh


def _dh_cdata_to_parameters(dh_cdata, backend):
    lib = backend._lib
    ffi = backend._ffi

    param_cdata = lib.DH_new()
    assert param_cdata != ffi.NULL
    param_cdata = ffi.gc(param_cdata, lib.DH_free)

    param_cdata.p = lib.BN_dup(dh_cdata.p)
    assert param_cdata.p != ffi.NULL

    param_cdata.g = lib.BN_dup(dh_cdata.g)
    assert param_cdata.g != ffi.NULL

    return _DHParameters(backend, param_cdata)


@utils.register_interface(dh.DHParametersWithSerialization)
class _DHParameters(object):
    def __init__(self, backend, dh_cdata):
        self._backend = backend
        self._dh_cdata = dh_cdata

    def parameter_numbers(self):
        return dh.DHParameterNumbers(
            p=self._backend._bn_to_int(self._dh_cdata.p),
            g=self._backend._bn_to_int(self._dh_cdata.g)
        )

    def generate_private_key(self):
        return self._backend.generate_dh_private_key(self)


@utils.register_interface(dh.DHPrivateKeyWithSerialization)
class _DHPrivateKey(object):
    def __init__(self, backend, dh_cdata):
        self._backend = backend
        self._dh_cdata = dh_cdata
        self._key_size = self._backend._lib.DH_size(dh_cdata) * 8

    @property
    def key_size(self):
        return self._key_size

    def private_numbers(self):
        return dh.DHPrivateNumbers(
            public_numbers=dh.DHPublicNumbers(
                parameter_numbers=dh.DHParameterNumbers(
                    p=self._backend._bn_to_int(self._dh_cdata.p),
                    g=self._backend._bn_to_int(self._dh_cdata.g)
                ),
                y=self._backend._bn_to_int(self._dh_cdata.pub_key)
            ),
            x=self._backend._bn_to_int(self._dh_cdata.priv_key)
        )

    def exchange(self):
        return _DHKeyExchangeContext(self)

    def public_key(self):
        dh_cdata = self._backend._lib.DH_new()
        assert dh_cdata != self._backend._ffi.NULL
        dh_cdata = self._backend._ffi.gc(
            dh_cdata, self._backend._lib.DH_free
        )
        dh_cdata.p = self._backend._lib.BN_dup(self._dh_cdata.p)
        dh_cdata.g = self._backend._lib.BN_dup(self._dh_cdata.g)
        dh_cdata.pub_key = self._backend._lib.BN_dup(self._dh_cdata.pub_key)
        return _DHPublicKey(self._backend, dh_cdata)

    def parameters(self):
        return _dh_cdata_to_parameters(self._dh_cdata, self._backend)


def _handle_dh_compute_key_error(errors, backend):
    lib = backend._lib

    assert errors[0][1:] == (
        lib.ERR_LIB_DH,
        lib.DH_F_COMPUTE_KEY,
        lib.DH_R_INVALID_PUBKEY
    )

    raise ValueError("Public key value is invalid for this exchange.")


def _agree_key(private_key, public_value, backend):
    lib = backend._lib
    ffi = backend._ffi

    key_size = lib.DH_size(private_key._dh_cdata)

    buf = ffi.new("char[]", key_size)
    res = lib.DH_compute_key(
        buf,
        backend._int_to_bn(public_value),
        private_key._dh_cdata
    )

    if res == -1:
        errors = backend._consume_errors()
        return _handle_dh_compute_key_error(errors, backend)
    else:
        assert res >= 1

        key = ffi.buffer(buf)[:res]
        pad = key_size - len(key)

        if pad > 0:
            key = (b"\x00" * pad) + key

        return key


class _DHKeyExchangeContext(object):
    def __init__(self, private_key):
        self._private_key = private_key
        self._backend = private_key._backend

    def agree(self, public_value):
        return _agree_key(
            self._private_key,
            public_value,
            self._backend
        )


@utils.register_interface(dh.DHPublicKeyWithSerialization)
class _DHPublicKey(object):
    def __init__(self, backend, dh_cdata):
        self._backend = backend
        self._dh_cdata = dh_cdata
        self._key_size = self._backend._lib.DH_size(dh_cdata) * 8

    @property
    def key_size(self):
        return self._key_size

    def public_numbers(self):
        return dh.DHPublicNumbers(
            parameter_numbers=dh.DHParameterNumbers(
                p=self._backend._bn_to_int(self._dh_cdata.p),
                g=self._backend._bn_to_int(self._dh_cdata.g)
            ),
            y=self._backend._bn_to_int(self._dh_cdata.pub_key)
        )

    def parameters(self):
        return _dh_cdata_to_parameters(self._dh_cdata, self._backend)

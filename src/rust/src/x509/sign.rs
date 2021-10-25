// This file is dual licensed under the terms of the Apache License, Version
// 2.0, and the BSD License. See the LICENSE file in the root of this repository
// for complete details.

use crate::x509;

lazy_static::lazy_static! {
    static ref ECDSA_WITH_SHA256_OID: asn1::ObjectIdentifier<'static> = asn1::ObjectIdentifier::from_string("1.2.840.10045.4.3.2").unwrap();
    static ref ECDSA_WITH_SHA384_OID: asn1::ObjectIdentifier<'static> = asn1::ObjectIdentifier::from_string("1.2.840.10045.4.3.3").unwrap();

    static ref ED25519_OID: asn1::ObjectIdentifier<'static> = asn1::ObjectIdentifier::from_string("1.3.101.112").unwrap();
    static ref ED448_OID: asn1::ObjectIdentifier<'static> = asn1::ObjectIdentifier::from_string("1.3.101.113").unwrap();
}

enum KeyType {
    Ec,
    Ed25519,
    Ed448,
}

enum HashType {
    None,
    Sha256,
    Sha384,
}

fn identify_key_type(py: pyo3::Python<'_>, private_key: &pyo3::PyAny) -> pyo3::PyResult<KeyType> {
    let ec_key_type: &pyo3::types::PyType = py
        .import("cryptography.hazmat.primitives.asymmetric.ec")?
        .getattr("EllipticCurvePrivateKey")?
        .extract()?;
    let ed25519_key_type: &pyo3::types::PyType = py
        .import("cryptography.hazmat.primitives.asymmetric.ed25519")?
        .getattr("Ed25519PrivateKey")?
        .extract()?;
    let ed448_key_type: &pyo3::types::PyType = py
        .import("cryptography.hazmat.primitives.asymmetric.ed448")?
        .getattr("Ed448PrivateKey")?
        .extract()?;

    if ec_key_type.is_instance(private_key)? {
        Ok(KeyType::Ec)
    } else if ed25519_key_type.is_instance(private_key)? {
        Ok(KeyType::Ed25519)
    } else if ed448_key_type.is_instance(private_key)? {
        Ok(KeyType::Ed448)
    } else {
        Err(pyo3::exceptions::PyTypeError::new_err(
            "Key must be an rsa, dsa, ec, ed25519, or ed448 private key.",
        ))
    }
}

fn identify_hash_type(
    py: pyo3::Python<'_>,
    hash_algorithm: &pyo3::PyAny,
) -> pyo3::PyResult<HashType> {
    if hash_algorithm.is_none() {
        return Ok(HashType::None);
    }

    let hash_algorithm_type: &pyo3::types::PyType = py
        .import("cryptography.hazmat.primitives.hashes")?
        .getattr("HashAlgorithm")?
        .extract()?;
    if !hash_algorithm_type.is_instance(hash_algorithm)? {
        return Err(pyo3::exceptions::PyTypeError::new_err(
            "Algorithm must be a registered hash algorithm.",
        ));
    }

    match hash_algorithm.getattr("name")?.extract()? {
        "sha256" => Ok(HashType::Sha256),
        "sha384" => Ok(HashType::Sha384),
        name => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "Hash algorithm {:?} not supported for signatures",
            name
        ))),
    }
}

pub(crate) fn compute_signature_algorithm<'p>(
    py: pyo3::Python<'p>,
    private_key: &'p pyo3::PyAny,
    hash_algorithm: &'p pyo3::PyAny,
) -> pyo3::PyResult<x509::AlgorithmIdentifier<'static>> {
    let key_type = identify_key_type(py, private_key)?;
    let hash_type = identify_hash_type(py, hash_algorithm)?;

    match (key_type, hash_type) {
        (KeyType::Ed25519, HashType::None) => Ok(x509::AlgorithmIdentifier {
            oid: (*ED25519_OID).clone(),
            params: None,
        }),
        (KeyType::Ed448, HashType::None) => Ok(x509::AlgorithmIdentifier {
            oid: (*ED448_OID).clone(),
            params: None,
        }),
        (KeyType::Ed25519, _) | (KeyType::Ed448, _) => {
            Err(pyo3::exceptions::PyValueError::new_err(
                "Algorithm must be None when signing via ed25519 or ed448",
            ))
        }

        (KeyType::Ec, HashType::Sha256) => Ok(x509::AlgorithmIdentifier {
            oid: (*ECDSA_WITH_SHA256_OID).clone(),
            params: None,
        }),
        (KeyType::Ec, HashType::Sha384) => Ok(x509::AlgorithmIdentifier {
            oid: (*ECDSA_WITH_SHA384_OID).clone(),
            params: None,
        }),

        (_, HashType::None) => Err(pyo3::exceptions::PyTypeError::new_err(
            "Algorithm must be a registered hash algorithm, not None.",
        )),
    }
}

pub(crate) fn sign_data<'p>(
    py: pyo3::Python<'p>,
    private_key: &'p pyo3::PyAny,
    hash_algorithm: &'p pyo3::PyAny,
    data: &[u8],
) -> pyo3::PyResult<&'p [u8]> {
    let key_type = identify_key_type(py, private_key)?;

    let signature = match key_type {
        KeyType::Ed25519 | KeyType::Ed448 => private_key.call_method1("sign", (data,))?,
        KeyType::Ec => {
            let ec_mod = py.import("cryptography.hazmat.primitives.asymmetric.ec")?;
            let ecdsa = ec_mod.getattr("ECDSA")?.call1((hash_algorithm,))?;
            private_key.call_method1("sign", (data, ecdsa))?
        }
    };
    signature.extract()
}

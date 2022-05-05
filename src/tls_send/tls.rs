use rustls;
use webpki_roots;

fn get_root_store() -> rustls::RootCertStore {
    let mut root_store = rustls::RootCertStore::empty();
    root_store.add_server_trust_anchors(
        webpki_roots::TLS_SERVER_ROOTS
            .0
            .iter()
            .map(|ta| {
                rustls::OwnedTrustAnchor::from_subject_spki_name_constraints(
                    ta.subject,
                    ta.spki,
                    ta.name_constraints
                )
            }
        )
    );
    root_store
}

pub fn get_client_config() -> rustls::ClientConfig {
    rustls::ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(get_root_store())
        .with_no_client_auth()
}
-- no-transaction

PRAGMA foreign_keys = OFF;

DROP INDEX IF EXISTS idx_dns_items_core_type;
DROP INDEX IF EXISTS idx_full_config_template_items_core_type;

DROP TABLE IF EXISTS profile_items_new;
CREATE TABLE profile_items_new (
    index_id TEXT PRIMARY KEY NOT NULL,
    config_type INTEGER NOT NULL,
    config_version INTEGER NOT NULL DEFAULT 4,
    subid TEXT NOT NULL DEFAULT '',
    is_sub INTEGER NOT NULL DEFAULT 1 CHECK (is_sub IN (0, 1)),
    pre_socks_port INTEGER,
    display_log INTEGER NOT NULL DEFAULT 1 CHECK (display_log IN (0, 1)),
    remarks TEXT NOT NULL DEFAULT '',
    address TEXT NOT NULL DEFAULT '',
    port INTEGER NOT NULL DEFAULT 0,
    password TEXT NOT NULL DEFAULT '',
    username TEXT NOT NULL DEFAULT '',
    network TEXT NOT NULL DEFAULT '',
    stream_security TEXT NOT NULL DEFAULT '',
    allow_insecure TEXT NOT NULL DEFAULT '',
    sni TEXT NOT NULL DEFAULT '',
    alpn TEXT NOT NULL DEFAULT '',
    fingerprint TEXT NOT NULL DEFAULT '',
    public_key TEXT NOT NULL DEFAULT '',
    short_id TEXT NOT NULL DEFAULT '',
    spider_x TEXT NOT NULL DEFAULT '',
    mldsa65_verify TEXT NOT NULL DEFAULT '',
    mux_enabled INTEGER CHECK (mux_enabled IN (0, 1)),
    cert TEXT NOT NULL DEFAULT '',
    cert_sha TEXT NOT NULL DEFAULT '',
    ech_config_list TEXT NOT NULL DEFAULT '',
    finalmask TEXT NOT NULL DEFAULT '',
    protocol_extra TEXT NOT NULL DEFAULT '{}',
    transport_extra TEXT NOT NULL DEFAULT '{}'
);

INSERT INTO profile_items_new (
    index_id, config_type, config_version, subid, is_sub,
    pre_socks_port, display_log, remarks, address, port, password,
    username, network, stream_security, allow_insecure, sni, alpn,
    fingerprint, public_key, short_id, spider_x, mldsa65_verify,
    mux_enabled, cert, cert_sha, ech_config_list, finalmask,
    protocol_extra, transport_extra
)
SELECT
    index_id, config_type, config_version, subid, is_sub,
    pre_socks_port, display_log, remarks, address, port, password,
    username, network, stream_security, allow_insecure, sni, alpn,
    fingerprint, public_key, short_id, spider_x, mldsa65_verify,
    mux_enabled, cert, cert_sha, ech_config_list, finalmask,
    protocol_extra, transport_extra
FROM profile_items;

DROP TABLE profile_items;
ALTER TABLE profile_items_new RENAME TO profile_items;
CREATE INDEX idx_profile_items_subid ON profile_items (subid);
CREATE INDEX idx_profile_items_config_type ON profile_items (config_type);

DROP TABLE IF EXISTS dns_items_new;
CREATE TABLE dns_items_new (
    id TEXT PRIMARY KEY NOT NULL,
    remarks TEXT NOT NULL DEFAULT '',
    enabled INTEGER NOT NULL DEFAULT 0 CHECK (enabled IN (0, 1)),
    use_system_hosts INTEGER NOT NULL DEFAULT 0 CHECK (use_system_hosts IN (0, 1)),
    normal_dns TEXT,
    tun_dns TEXT,
    domain_strategy4_freedom TEXT,
    domain_dns_address TEXT
);

INSERT INTO dns_items_new (
    id, remarks, enabled, use_system_hosts, normal_dns,
    tun_dns, domain_strategy4_freedom, domain_dns_address
)
SELECT
    id, remarks, enabled, use_system_hosts, normal_dns,
    tun_dns, domain_strategy4_freedom, domain_dns_address
FROM dns_items;

DROP TABLE dns_items;
ALTER TABLE dns_items_new RENAME TO dns_items;

DROP TABLE IF EXISTS full_config_template_items_new;
CREATE TABLE full_config_template_items_new (
    id TEXT PRIMARY KEY NOT NULL,
    remarks TEXT NOT NULL DEFAULT '',
    enabled INTEGER NOT NULL DEFAULT 0 CHECK (enabled IN (0, 1)),
    config TEXT,
    tun_config TEXT,
    add_proxy_only INTEGER CHECK (add_proxy_only IN (0, 1)),
    proxy_detour TEXT
);

INSERT INTO full_config_template_items_new (
    id, remarks, enabled, config, tun_config,
    add_proxy_only, proxy_detour
)
SELECT
    id, remarks, enabled, config, tun_config,
    add_proxy_only, proxy_detour
FROM full_config_template_items;

DROP TABLE full_config_template_items;
ALTER TABLE full_config_template_items_new RENAME TO full_config_template_items;

PRAGMA foreign_keys = ON;

ALTER TABLE profile_ex_items ADD COLUMN country_code TEXT;

CREATE TRIGGER clear_country_on_connection_change
AFTER UPDATE OF protocol, transport, tls ON profile_items
WHEN OLD.protocol IS NOT NEW.protocol OR OLD.transport IS NOT NEW.transport OR OLD.tls IS NOT NEW.tls
BEGIN
    UPDATE profile_ex_items SET country_code = NULL, delay = 0, message = NULL, ip_info = NULL WHERE index_id = NEW.index_id;
END;

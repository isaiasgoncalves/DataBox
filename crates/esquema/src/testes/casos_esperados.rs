//! O que cada caso de `referencia.rs` deve devolver (plano ou erro). `CASOS[i].setup` roda antes, num banco de
//! exemplo novo.

pub(super) enum Esperado {
    Plano {
        descricao: &'static str,
        comandos: &'static [&'static str],
        reconstroi: bool,
        avisos: &'static [&'static str],
    },
    Erro {
        mensagem: &'static str,
        status: u16,
    },
    Feito,
}

pub(super) struct Caso {
    pub nome: &'static str,
    pub setup: &'static str,
    pub esperado: Esperado,
}

pub(super) const CASOS: &[Caso] = &[
    Caso {
        nome: "alterar_pedidos_observacao_varchar",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Mudar a coluna observacao de pedidos: tipo TEXT → VARCHAR(1000). A tabela será reconstruída.",
            comandos: &[
                "BEGIN",
                "CREATE TABLE \"_novo_pedidos\" (\n    \"id\" INTEGER PRIMARY KEY,\n    \"cliente_id\" INTEGER NOT NULL REFERENCES \"clientes\" (\"id\") ON DELETE CASCADE,\n    \"feito_em\" TEXT NOT NULL,\n    \"status\" TEXT NOT NULL CHECK (status IN ('aberto', 'pago', 'cancelado')) DEFAULT 'aberto',\n    \"observacao\" VARCHAR(1000)\n)",
                "INSERT INTO \"_novo_pedidos\" (\"id\", \"cliente_id\", \"feito_em\", \"status\", \"observacao\")\n    SELECT \"id\", \"cliente_id\", \"feito_em\", \"status\", \"observacao\" FROM \"pedidos\"",
                "DROP VIEW \"v_pedidos_por_cliente\"",
                "DROP TABLE \"pedidos\"",
                "ALTER TABLE \"_novo_pedidos\" RENAME TO \"pedidos\"",
                "CREATE INDEX idx_pedidos_cliente ON pedidos (cliente_id)",
                "CREATE INDEX idx_pedidos_status_data ON pedidos (status, feito_em DESC)",
                "CREATE TRIGGER trg_pedido_cancelado AFTER UPDATE OF status ON pedidos\n    WHEN new.status = 'cancelado'\nBEGIN\n    UPDATE pedidos SET observacao = coalesce(observacao || ' · ', '') || 'cancelado' WHERE id = new.id;\nEND",
                "CREATE VIEW v_pedidos_por_cliente AS\n    SELECT c.id, c.nome, count(p.id) AS pedidos, max(p.feito_em) AS ultimo\n    FROM clientes c LEFT JOIN pedidos p ON p.cliente_id = c.id\n    GROUP BY c.id",
                "PRAGMA foreign_key_check(\"pedidos\")",
                "PRAGMA foreign_key_check(\"itens_pedido\")",
                "COMMIT",
            ],
            reconstroi: true,
            avisos: &[
                "A tabela será reconstruída: os dados são copiados para uma tabela nova com a definição alterada, e os índices, gatilhos e visões ligados a ela são recriados.",
                "Durante a reconstrução as chaves estrangeiras ficam desligadas (PRAGMA foreign_keys = OFF) e o ALTER TABLE usa o modo antigo (PRAGMA legacy_alter_table = ON); os dois voltam como estavam no fim.",
            ],
        },
    },
    Caso {
        nome: "alterar_produtos_preco_real",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Mudar a coluna preco de produtos: tipo NUMERIC → REAL. A tabela será reconstruída.",
            comandos: &[
                "BEGIN",
                "CREATE TABLE \"_novo_produtos\" (\n    \"codigo\" TEXT PRIMARY KEY,\n    \"descricao\" TEXT NOT NULL COLLATE NOCASE,\n    \"preco\" REAL NOT NULL DEFAULT 0,\n    \"estoque\" INTEGER DEFAULT 0,\n    \"foto\" BLOB,\n    \"serie\" INTEGER,\n    \"preco_centavos\" INTEGER GENERATED ALWAYS AS (CAST(preco * 100 AS INTEGER)) VIRTUAL\n) WITHOUT ROWID",
                "INSERT INTO \"_novo_produtos\" (\"codigo\", \"descricao\", \"preco\", \"estoque\", \"foto\", \"serie\")\n    SELECT \"codigo\", \"descricao\", \"preco\", \"estoque\", \"foto\", \"serie\" FROM \"produtos\"",
                "DROP TABLE \"produtos\"",
                "ALTER TABLE \"_novo_produtos\" RENAME TO \"produtos\"",
                "PRAGMA foreign_key_check(\"produtos\")",
                "PRAGMA foreign_key_check(\"itens_pedido\")",
                "COMMIT",
            ],
            reconstroi: true,
            avisos: &[
                "A tabela será reconstruída: os dados são copiados para uma tabela nova com a definição alterada, e os índices, gatilhos e visões ligados a ela são recriados.",
                "Os valores são copiados e convertidos pela afinidade do tipo novo (REAL).",
                "Durante a reconstrução as chaves estrangeiras ficam desligadas (PRAGMA foreign_keys = OFF) e o ALTER TABLE usa o modo antigo (PRAGMA legacy_alter_table = ON); os dois voltam como estavam no fim.",
            ],
        },
    },
    Caso {
        nome: "alterar_clientes_cidade_nao_nulo",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Mudar a coluna cidade de clientes: passa a não aceitar nulo. A tabela será reconstruída.",
            comandos: &[
                "BEGIN",
                "CREATE TABLE \"_novo_clientes\" (\n    \"id\" INTEGER PRIMARY KEY AUTOINCREMENT,\n    \"nome\" TEXT NOT NULL,\n    \"email\" TEXT UNIQUE,\n    \"cidade\" TEXT NOT NULL DEFAULT 'São Paulo',\n    \"limite\" REAL CHECK (limite >= 0),\n    \"ativo\" INTEGER NOT NULL DEFAULT 1,\n    \"criado_em\" TEXT DEFAULT CURRENT_TIMESTAMP\n)",
                "INSERT INTO \"_novo_clientes\" (\"id\", \"nome\", \"email\", \"cidade\", \"limite\", \"ativo\", \"criado_em\")\n    SELECT \"id\", \"nome\", \"email\", \"cidade\", \"limite\", \"ativo\", \"criado_em\" FROM \"clientes\"",
                "DELETE FROM sqlite_sequence WHERE name = '_novo_clientes'",
                "INSERT INTO sqlite_sequence (name, seq) SELECT '_novo_clientes', seq FROM sqlite_sequence WHERE name = 'clientes'",
                "DROP VIEW \"v_pedidos_por_cliente\"",
                "DROP TABLE \"clientes\"",
                "ALTER TABLE \"_novo_clientes\" RENAME TO \"clientes\"",
                "CREATE UNIQUE INDEX idx_clientes_nome_cidade ON clientes (nome, cidade) WHERE ativo = 1",
                "CREATE VIEW v_pedidos_por_cliente AS\n    SELECT c.id, c.nome, count(p.id) AS pedidos, max(p.feito_em) AS ultimo\n    FROM clientes c LEFT JOIN pedidos p ON p.cliente_id = c.id\n    GROUP BY c.id",
                "PRAGMA foreign_key_check(\"clientes\")",
                "PRAGMA foreign_key_check(\"pedidos\")",
                "COMMIT",
            ],
            reconstroi: true,
            avisos: &[
                "A tabela será reconstruída: os dados são copiados para uma tabela nova com a definição alterada, e os índices, gatilhos e visões ligados a ela são recriados.",
                "Durante a reconstrução as chaves estrangeiras ficam desligadas (PRAGMA foreign_keys = OFF) e o ALTER TABLE usa o modo antigo (PRAGMA legacy_alter_table = ON); os dois voltam como estavam no fim.",
            ],
        },
    },
    Caso {
        nome: "alterar_clientes_limite_nao_nulo",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Mudar a coluna limite de clientes: passa a não aceitar nulo. A tabela será reconstruída.",
            comandos: &[
                "BEGIN",
                "CREATE TABLE \"_novo_clientes\" (\n    \"id\" INTEGER PRIMARY KEY AUTOINCREMENT,\n    \"nome\" TEXT NOT NULL,\n    \"email\" TEXT UNIQUE,\n    \"cidade\" TEXT DEFAULT 'São Paulo',\n    \"limite\" REAL NOT NULL CHECK (limite >= 0),\n    \"ativo\" INTEGER NOT NULL DEFAULT 1,\n    \"criado_em\" TEXT DEFAULT CURRENT_TIMESTAMP\n)",
                "INSERT INTO \"_novo_clientes\" (\"id\", \"nome\", \"email\", \"cidade\", \"limite\", \"ativo\", \"criado_em\")\n    SELECT \"id\", \"nome\", \"email\", \"cidade\", \"limite\", \"ativo\", \"criado_em\" FROM \"clientes\"",
                "DELETE FROM sqlite_sequence WHERE name = '_novo_clientes'",
                "INSERT INTO sqlite_sequence (name, seq) SELECT '_novo_clientes', seq FROM sqlite_sequence WHERE name = 'clientes'",
                "DROP VIEW \"v_pedidos_por_cliente\"",
                "DROP TABLE \"clientes\"",
                "ALTER TABLE \"_novo_clientes\" RENAME TO \"clientes\"",
                "CREATE UNIQUE INDEX idx_clientes_nome_cidade ON clientes (nome, cidade) WHERE ativo = 1",
                "CREATE VIEW v_pedidos_por_cliente AS\n    SELECT c.id, c.nome, count(p.id) AS pedidos, max(p.feito_em) AS ultimo\n    FROM clientes c LEFT JOIN pedidos p ON p.cliente_id = c.id\n    GROUP BY c.id",
                "PRAGMA foreign_key_check(\"clientes\")",
                "PRAGMA foreign_key_check(\"pedidos\")",
                "COMMIT",
            ],
            reconstroi: true,
            avisos: &[
                "A tabela será reconstruída: os dados são copiados para uma tabela nova com a definição alterada, e os índices, gatilhos e visões ligados a ela são recriados.",
                "Há 8 linhas com limite nulo; linhas com valor nulo impedem NOT NULL, e a mudança vai falhar enquanto elas existirem.",
                "Durante a reconstrução as chaves estrangeiras ficam desligadas (PRAGMA foreign_keys = OFF) e o ALTER TABLE usa o modo antigo (PRAGMA legacy_alter_table = ON); os dois voltam como estavam no fim.",
            ],
        },
    },
    Caso {
        nome: "alterar_clientes_email_sem_unica",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Mudar a coluna email de clientes: passa a aceitar repetição. A tabela será reconstruída.",
            comandos: &[
                "BEGIN",
                "CREATE TABLE \"_novo_clientes\" (\n    \"id\" INTEGER PRIMARY KEY AUTOINCREMENT,\n    \"nome\" TEXT NOT NULL,\n    \"email\" TEXT,\n    \"cidade\" TEXT DEFAULT 'São Paulo',\n    \"limite\" REAL CHECK (limite >= 0),\n    \"ativo\" INTEGER NOT NULL DEFAULT 1,\n    \"criado_em\" TEXT DEFAULT CURRENT_TIMESTAMP\n)",
                "INSERT INTO \"_novo_clientes\" (\"id\", \"nome\", \"email\", \"cidade\", \"limite\", \"ativo\", \"criado_em\")\n    SELECT \"id\", \"nome\", \"email\", \"cidade\", \"limite\", \"ativo\", \"criado_em\" FROM \"clientes\"",
                "DELETE FROM sqlite_sequence WHERE name = '_novo_clientes'",
                "INSERT INTO sqlite_sequence (name, seq) SELECT '_novo_clientes', seq FROM sqlite_sequence WHERE name = 'clientes'",
                "DROP VIEW \"v_pedidos_por_cliente\"",
                "DROP TABLE \"clientes\"",
                "ALTER TABLE \"_novo_clientes\" RENAME TO \"clientes\"",
                "CREATE UNIQUE INDEX idx_clientes_nome_cidade ON clientes (nome, cidade) WHERE ativo = 1",
                "CREATE VIEW v_pedidos_por_cliente AS\n    SELECT c.id, c.nome, count(p.id) AS pedidos, max(p.feito_em) AS ultimo\n    FROM clientes c LEFT JOIN pedidos p ON p.cliente_id = c.id\n    GROUP BY c.id",
                "PRAGMA foreign_key_check(\"clientes\")",
                "PRAGMA foreign_key_check(\"pedidos\")",
                "COMMIT",
            ],
            reconstroi: true,
            avisos: &[
                "A tabela será reconstruída: os dados são copiados para uma tabela nova com a definição alterada, e os índices, gatilhos e visões ligados a ela são recriados.",
                "Durante a reconstrução as chaves estrangeiras ficam desligadas (PRAGMA foreign_keys = OFF) e o ALTER TABLE usa o modo antigo (PRAGMA legacy_alter_table = ON); os dois voltam como estavam no fim.",
            ],
        },
    },
    Caso {
        nome: "alterar_clientes_nome_unica",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Mudar a coluna nome de clientes: passa a não aceitar repetição (UNIQUE). A tabela será reconstruída.",
            comandos: &[
                "BEGIN",
                "CREATE TABLE \"_novo_clientes\" (\n    \"id\" INTEGER PRIMARY KEY AUTOINCREMENT,\n    \"nome\" TEXT NOT NULL UNIQUE,\n    \"email\" TEXT UNIQUE,\n    \"cidade\" TEXT DEFAULT 'São Paulo',\n    \"limite\" REAL CHECK (limite >= 0),\n    \"ativo\" INTEGER NOT NULL DEFAULT 1,\n    \"criado_em\" TEXT DEFAULT CURRENT_TIMESTAMP\n)",
                "INSERT INTO \"_novo_clientes\" (\"id\", \"nome\", \"email\", \"cidade\", \"limite\", \"ativo\", \"criado_em\")\n    SELECT \"id\", \"nome\", \"email\", \"cidade\", \"limite\", \"ativo\", \"criado_em\" FROM \"clientes\"",
                "DELETE FROM sqlite_sequence WHERE name = '_novo_clientes'",
                "INSERT INTO sqlite_sequence (name, seq) SELECT '_novo_clientes', seq FROM sqlite_sequence WHERE name = 'clientes'",
                "DROP VIEW \"v_pedidos_por_cliente\"",
                "DROP TABLE \"clientes\"",
                "ALTER TABLE \"_novo_clientes\" RENAME TO \"clientes\"",
                "CREATE UNIQUE INDEX idx_clientes_nome_cidade ON clientes (nome, cidade) WHERE ativo = 1",
                "CREATE VIEW v_pedidos_por_cliente AS\n    SELECT c.id, c.nome, count(p.id) AS pedidos, max(p.feito_em) AS ultimo\n    FROM clientes c LEFT JOIN pedidos p ON p.cliente_id = c.id\n    GROUP BY c.id",
                "PRAGMA foreign_key_check(\"clientes\")",
                "PRAGMA foreign_key_check(\"pedidos\")",
                "COMMIT",
            ],
            reconstroi: true,
            avisos: &[
                "A tabela será reconstruída: os dados são copiados para uma tabela nova com a definição alterada, e os índices, gatilhos e visões ligados a ela são recriados.",
                "Há 8 valores repetidos em nome; a mudança vai falhar enquanto houver repetição.",
                "Durante a reconstrução as chaves estrangeiras ficam desligadas (PRAGMA foreign_keys = OFF) e o ALTER TABLE usa o modo antigo (PRAGMA legacy_alter_table = ON); os dois voltam como estavam no fim.",
            ],
        },
    },
    Caso {
        nome: "alterar_pedidos_status_renomear_e_tipo",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Mudar a coluna status de pedidos: nome status → situacao; tipo TEXT → VARCHAR(20). A tabela será reconstruída.",
            comandos: &[
                "BEGIN",
                "ALTER TABLE \"pedidos\" RENAME COLUMN \"status\" TO \"situacao\"",
                "CREATE TABLE \"_novo_pedidos\" (\n    \"id\" INTEGER PRIMARY KEY,\n    \"cliente_id\" INTEGER NOT NULL REFERENCES \"clientes\" (\"id\") ON DELETE CASCADE,\n    \"feito_em\" TEXT NOT NULL,\n    \"situacao\" VARCHAR(20) NOT NULL CHECK (\"situacao\" IN ('aberto', 'pago', 'cancelado')) DEFAULT 'aberto',\n    \"observacao\" TEXT\n)",
                "INSERT INTO \"_novo_pedidos\" (\"id\", \"cliente_id\", \"feito_em\", \"situacao\", \"observacao\")\n    SELECT \"id\", \"cliente_id\", \"feito_em\", \"situacao\", \"observacao\" FROM \"pedidos\"",
                "DROP VIEW \"v_pedidos_por_cliente\"",
                "DROP TABLE \"pedidos\"",
                "ALTER TABLE \"_novo_pedidos\" RENAME TO \"pedidos\"",
                "CREATE INDEX idx_pedidos_cliente ON pedidos (cliente_id)",
                "CREATE INDEX idx_pedidos_status_data ON pedidos (\"situacao\", feito_em DESC)",
                "CREATE TRIGGER trg_pedido_cancelado AFTER UPDATE OF \"situacao\" ON pedidos\n    WHEN new.\"situacao\" = 'cancelado'\nBEGIN\n    UPDATE pedidos SET observacao = coalesce(observacao || ' · ', '') || 'cancelado' WHERE id = new.id;\nEND",
                "CREATE VIEW v_pedidos_por_cliente AS\n    SELECT c.id, c.nome, count(p.id) AS pedidos, max(p.feito_em) AS ultimo\n    FROM clientes c LEFT JOIN pedidos p ON p.cliente_id = c.id\n    GROUP BY c.id",
                "PRAGMA foreign_key_check(\"pedidos\")",
                "PRAGMA foreign_key_check(\"itens_pedido\")",
                "COMMIT",
            ],
            reconstroi: true,
            avisos: &[
                "A tabela será reconstruída: os dados são copiados para uma tabela nova com a definição alterada, e os índices, gatilhos e visões ligados a ela são recriados.",
                "Durante a reconstrução as chaves estrangeiras ficam desligadas (PRAGMA foreign_keys = OFF) e o ALTER TABLE usa o modo antigo (PRAGMA legacy_alter_table = ON); os dois voltam como estavam no fim.",
            ],
        },
    },
    Caso {
        nome: "alterar_pedidos_status_renomear",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Renomear a coluna status de pedidos para situacao.",
            comandos: &["ALTER TABLE \"pedidos\" RENAME COLUMN \"status\" TO \"situacao\""],
            reconstroi: false,
            avisos: &["Índices, gatilhos e visões que citam a coluna passam a usar o nome novo."],
        },
    },
    Caso {
        nome: "alterar_clientes_nome_nada_muda",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Nada muda na coluna nome.",
            comandos: &[],
            reconstroi: false,
            avisos: &[],
        },
    },
    Caso {
        nome: "alterar_clientes_nome_so_espacos",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Nada muda na coluna nome.",
            comandos: &[],
            reconstroi: false,
            avisos: &[],
        },
    },
    Caso {
        nome: "alterar_clientes_id_sem_pk",
        setup: "",
        esperado: Esperado::Erro { mensagem: "Autoincremento só vale numa chave primária INTEGER.", status: 400 },
    },
    Caso {
        nome: "alterar_clientes_id_sem_pk_nem_auto",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Mudar a coluna id de clientes: deixa de ser chave primária; perde o autoincremento. A tabela será reconstruída.",
            comandos: &[
                "BEGIN",
                "CREATE TABLE \"_novo_clientes\" (\n    \"id\" INTEGER,\n    \"nome\" TEXT NOT NULL,\n    \"email\" TEXT UNIQUE,\n    \"cidade\" TEXT DEFAULT 'São Paulo',\n    \"limite\" REAL CHECK (limite >= 0),\n    \"ativo\" INTEGER NOT NULL DEFAULT 1,\n    \"criado_em\" TEXT DEFAULT CURRENT_TIMESTAMP\n)",
                "INSERT INTO \"_novo_clientes\" (rowid, \"id\", \"nome\", \"email\", \"cidade\", \"limite\", \"ativo\", \"criado_em\")\n    SELECT rowid, \"id\", \"nome\", \"email\", \"cidade\", \"limite\", \"ativo\", \"criado_em\" FROM \"clientes\"",
                "DROP VIEW \"v_pedidos_por_cliente\"",
                "DROP TABLE \"clientes\"",
                "ALTER TABLE \"_novo_clientes\" RENAME TO \"clientes\"",
                "CREATE UNIQUE INDEX idx_clientes_nome_cidade ON clientes (nome, cidade) WHERE ativo = 1",
                "CREATE VIEW v_pedidos_por_cliente AS\n    SELECT c.id, c.nome, count(p.id) AS pedidos, max(p.feito_em) AS ultimo\n    FROM clientes c LEFT JOIN pedidos p ON p.cliente_id = c.id\n    GROUP BY c.id",
                "PRAGMA foreign_key_check(\"clientes\")",
                "PRAGMA foreign_key_check(\"pedidos\")",
                "COMMIT",
            ],
            reconstroi: true,
            avisos: &[
                "A tabela será reconstruída: os dados são copiados para uma tabela nova com a definição alterada, e os índices, gatilhos e visões ligados a ela são recriados.",
                "Outras tabelas têm chave estrangeira para esta; se alguma ficar sem correspondência, a verificação no fim desfaz tudo.",
                "Durante a reconstrução as chaves estrangeiras ficam desligadas (PRAGMA foreign_keys = OFF) e o ALTER TABLE usa o modo antigo (PRAGMA legacy_alter_table = ON); os dois voltam como estavam no fim.",
            ],
        },
    },
    Caso {
        nome: "alterar_eventos_valor_gerada",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Mudar a coluna valor de eventos: expressão gerada nenhuma → length(dados) (VIRTUAL). A tabela será reconstruída.",
            comandos: &[
                "BEGIN",
                "CREATE TABLE \"_novo_eventos\" (\n    \"momento\" TEXT,\n    \"tipo\" TEXT,\n    \"valor\" REAL GENERATED ALWAYS AS (length(dados)) VIRTUAL,\n    \"dados\" TEXT\n)",
                "INSERT INTO \"_novo_eventos\" (rowid, \"momento\", \"tipo\", \"dados\")\n    SELECT rowid, \"momento\", \"tipo\", \"dados\" FROM \"eventos\"",
                "DROP TABLE \"eventos\"",
                "ALTER TABLE \"_novo_eventos\" RENAME TO \"eventos\"",
                "PRAGMA foreign_key_check(\"eventos\")",
                "COMMIT",
            ],
            reconstroi: true,
            avisos: &[
                "A tabela será reconstruída: os dados são copiados para uma tabela nova com a definição alterada, e os índices, gatilhos e visões ligados a ela são recriados.",
                "A coluna passa a ser calculada: os valores gravados nela se perdem.",
                "Durante a reconstrução as chaves estrangeiras ficam desligadas (PRAGMA foreign_keys = OFF) e o ALTER TABLE usa o modo antigo (PRAGMA legacy_alter_table = ON); os dois voltam como estavam no fim.",
            ],
        },
    },
    Caso {
        nome: "alterar_produtos_centavos_sem_gerada",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Mudar a coluna preco_centavos de produtos: expressão gerada CAST(preco * 100 AS INTEGER) (VIRTUAL) → nenhuma. A tabela será reconstruída.",
            comandos: &[
                "BEGIN",
                "CREATE TABLE \"_novo_produtos\" (\n    \"codigo\" TEXT PRIMARY KEY,\n    \"descricao\" TEXT NOT NULL COLLATE NOCASE,\n    \"preco\" NUMERIC NOT NULL DEFAULT 0,\n    \"estoque\" INTEGER DEFAULT 0,\n    \"foto\" BLOB,\n    \"serie\" INTEGER,\n    \"preco_centavos\" INTEGER\n) WITHOUT ROWID",
                "INSERT INTO \"_novo_produtos\" (\"codigo\", \"descricao\", \"preco\", \"estoque\", \"foto\", \"serie\", \"preco_centavos\")\n    SELECT \"codigo\", \"descricao\", \"preco\", \"estoque\", \"foto\", \"serie\", \"preco_centavos\" FROM \"produtos\"",
                "DROP TABLE \"produtos\"",
                "ALTER TABLE \"_novo_produtos\" RENAME TO \"produtos\"",
                "PRAGMA foreign_key_check(\"produtos\")",
                "PRAGMA foreign_key_check(\"itens_pedido\")",
                "COMMIT",
            ],
            reconstroi: true,
            avisos: &[
                "A tabela será reconstruída: os dados são copiados para uma tabela nova com a definição alterada, e os índices, gatilhos e visões ligados a ela são recriados.",
                "Durante a reconstrução as chaves estrangeiras ficam desligadas (PRAGMA foreign_keys = OFF) e o ALTER TABLE usa o modo antigo (PRAGMA legacy_alter_table = ON); os dois voltam como estavam no fim.",
            ],
        },
    },
    Caso {
        nome: "alterar_itens_quantidade_check",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Mudar a coluna quantidade de itens_pedido: verificação nenhuma → quantidade > 0. A tabela será reconstruída.",
            comandos: &[
                "BEGIN",
                "CREATE TABLE \"_novo_itens_pedido\" (\n    \"pedido_id\" INTEGER NOT NULL REFERENCES \"pedidos\" (\"id\") ON DELETE CASCADE,\n    \"produto\" TEXT NOT NULL REFERENCES \"produtos\" (\"codigo\"),\n    \"quantidade\" INTEGER NOT NULL CHECK (quantidade > 0) DEFAULT 1,\n    \"preco\" NUMERIC,\n    PRIMARY KEY (pedido_id, produto)\n)",
                "INSERT INTO \"_novo_itens_pedido\" (rowid, \"pedido_id\", \"produto\", \"quantidade\", \"preco\")\n    SELECT rowid, \"pedido_id\", \"produto\", \"quantidade\", \"preco\" FROM \"itens_pedido\"",
                "DROP TABLE \"itens_pedido\"",
                "ALTER TABLE \"_novo_itens_pedido\" RENAME TO \"itens_pedido\"",
                "PRAGMA foreign_key_check(\"itens_pedido\")",
                "COMMIT",
            ],
            reconstroi: true,
            avisos: &[
                "A tabela será reconstruída: os dados são copiados para uma tabela nova com a definição alterada, e os índices, gatilhos e visões ligados a ela são recriados.",
                "Durante a reconstrução as chaves estrangeiras ficam desligadas (PRAGMA foreign_keys = OFF) e o ALTER TABLE usa o modo antigo (PRAGMA legacy_alter_table = ON); os dois voltam como estavam no fim.",
            ],
        },
    },
    Caso {
        nome: "alterar_clientes_cidade_padrao_colacao",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Mudar a coluna cidade de clientes: valor padrão 'São Paulo' → 'Recife'; colação nenhuma → NOCASE. A tabela será reconstruída.",
            comandos: &[
                "BEGIN",
                "CREATE TABLE \"_novo_clientes\" (\n    \"id\" INTEGER PRIMARY KEY AUTOINCREMENT,\n    \"nome\" TEXT NOT NULL,\n    \"email\" TEXT UNIQUE,\n    \"cidade\" TEXT DEFAULT 'Recife' COLLATE NOCASE,\n    \"limite\" REAL CHECK (limite >= 0),\n    \"ativo\" INTEGER NOT NULL DEFAULT 1,\n    \"criado_em\" TEXT DEFAULT CURRENT_TIMESTAMP\n)",
                "INSERT INTO \"_novo_clientes\" (\"id\", \"nome\", \"email\", \"cidade\", \"limite\", \"ativo\", \"criado_em\")\n    SELECT \"id\", \"nome\", \"email\", \"cidade\", \"limite\", \"ativo\", \"criado_em\" FROM \"clientes\"",
                "DELETE FROM sqlite_sequence WHERE name = '_novo_clientes'",
                "INSERT INTO sqlite_sequence (name, seq) SELECT '_novo_clientes', seq FROM sqlite_sequence WHERE name = 'clientes'",
                "DROP VIEW \"v_pedidos_por_cliente\"",
                "DROP TABLE \"clientes\"",
                "ALTER TABLE \"_novo_clientes\" RENAME TO \"clientes\"",
                "CREATE UNIQUE INDEX idx_clientes_nome_cidade ON clientes (nome, cidade) WHERE ativo = 1",
                "CREATE VIEW v_pedidos_por_cliente AS\n    SELECT c.id, c.nome, count(p.id) AS pedidos, max(p.feito_em) AS ultimo\n    FROM clientes c LEFT JOIN pedidos p ON p.cliente_id = c.id\n    GROUP BY c.id",
                "PRAGMA foreign_key_check(\"clientes\")",
                "PRAGMA foreign_key_check(\"pedidos\")",
                "COMMIT",
            ],
            reconstroi: true,
            avisos: &[
                "A tabela será reconstruída: os dados são copiados para uma tabela nova com a definição alterada, e os índices, gatilhos e visões ligados a ela são recriados.",
                "Durante a reconstrução as chaves estrangeiras ficam desligadas (PRAGMA foreign_keys = OFF) e o ALTER TABLE usa o modo antigo (PRAGMA legacy_alter_table = ON); os dois voltam como estavam no fim.",
            ],
        },
    },
    Caso {
        nome: "alterar_pedidos_cliente_ref",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Mudar a coluna cliente_id de pedidos: referência clientes.id (ao excluir CASCADE) → clientes.id (ao excluir SET NULL) (ao atualizar CASCADE). A tabela será reconstruída.",
            comandos: &[
                "BEGIN",
                "CREATE TABLE \"_novo_pedidos\" (\n    \"id\" INTEGER PRIMARY KEY,\n    \"cliente_id\" INTEGER NOT NULL REFERENCES \"clientes\" (\"id\") ON DELETE SET NULL ON UPDATE CASCADE,\n    \"feito_em\" TEXT NOT NULL,\n    \"status\" TEXT NOT NULL CHECK (status IN ('aberto', 'pago', 'cancelado')) DEFAULT 'aberto',\n    \"observacao\" TEXT\n)",
                "INSERT INTO \"_novo_pedidos\" (\"id\", \"cliente_id\", \"feito_em\", \"status\", \"observacao\")\n    SELECT \"id\", \"cliente_id\", \"feito_em\", \"status\", \"observacao\" FROM \"pedidos\"",
                "DROP VIEW \"v_pedidos_por_cliente\"",
                "DROP TABLE \"pedidos\"",
                "ALTER TABLE \"_novo_pedidos\" RENAME TO \"pedidos\"",
                "CREATE INDEX idx_pedidos_cliente ON pedidos (cliente_id)",
                "CREATE INDEX idx_pedidos_status_data ON pedidos (status, feito_em DESC)",
                "CREATE TRIGGER trg_pedido_cancelado AFTER UPDATE OF status ON pedidos\n    WHEN new.status = 'cancelado'\nBEGIN\n    UPDATE pedidos SET observacao = coalesce(observacao || ' · ', '') || 'cancelado' WHERE id = new.id;\nEND",
                "CREATE VIEW v_pedidos_por_cliente AS\n    SELECT c.id, c.nome, count(p.id) AS pedidos, max(p.feito_em) AS ultimo\n    FROM clientes c LEFT JOIN pedidos p ON p.cliente_id = c.id\n    GROUP BY c.id",
                "PRAGMA foreign_key_check(\"pedidos\")",
                "PRAGMA foreign_key_check(\"itens_pedido\")",
                "COMMIT",
            ],
            reconstroi: true,
            avisos: &[
                "A tabela será reconstruída: os dados são copiados para uma tabela nova com a definição alterada, e os índices, gatilhos e visões ligados a ela são recriados.",
                "Durante a reconstrução as chaves estrangeiras ficam desligadas (PRAGMA foreign_keys = OFF) e o ALTER TABLE usa o modo antigo (PRAGMA legacy_alter_table = ON); os dois voltam como estavam no fim.",
            ],
        },
    },
    Caso {
        nome: "alterar_pedidos_cliente_sem_ref",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Mudar a coluna cliente_id de pedidos: referência clientes.id (ao excluir CASCADE) → nenhuma. A tabela será reconstruída.",
            comandos: &[
                "BEGIN",
                "CREATE TABLE \"_novo_pedidos\" (\n    \"id\" INTEGER PRIMARY KEY,\n    \"cliente_id\" INTEGER NOT NULL,\n    \"feito_em\" TEXT NOT NULL,\n    \"status\" TEXT NOT NULL CHECK (status IN ('aberto', 'pago', 'cancelado')) DEFAULT 'aberto',\n    \"observacao\" TEXT\n)",
                "INSERT INTO \"_novo_pedidos\" (\"id\", \"cliente_id\", \"feito_em\", \"status\", \"observacao\")\n    SELECT \"id\", \"cliente_id\", \"feito_em\", \"status\", \"observacao\" FROM \"pedidos\"",
                "DROP VIEW \"v_pedidos_por_cliente\"",
                "DROP TABLE \"pedidos\"",
                "ALTER TABLE \"_novo_pedidos\" RENAME TO \"pedidos\"",
                "CREATE INDEX idx_pedidos_cliente ON pedidos (cliente_id)",
                "CREATE INDEX idx_pedidos_status_data ON pedidos (status, feito_em DESC)",
                "CREATE TRIGGER trg_pedido_cancelado AFTER UPDATE OF status ON pedidos\n    WHEN new.status = 'cancelado'\nBEGIN\n    UPDATE pedidos SET observacao = coalesce(observacao || ' · ', '') || 'cancelado' WHERE id = new.id;\nEND",
                "CREATE VIEW v_pedidos_por_cliente AS\n    SELECT c.id, c.nome, count(p.id) AS pedidos, max(p.feito_em) AS ultimo\n    FROM clientes c LEFT JOIN pedidos p ON p.cliente_id = c.id\n    GROUP BY c.id",
                "PRAGMA foreign_key_check(\"pedidos\")",
                "PRAGMA foreign_key_check(\"itens_pedido\")",
                "COMMIT",
            ],
            reconstroi: true,
            avisos: &[
                "A tabela será reconstruída: os dados são copiados para uma tabela nova com a definição alterada, e os índices, gatilhos e visões ligados a ela são recriados.",
                "Durante a reconstrução as chaves estrangeiras ficam desligadas (PRAGMA foreign_keys = OFF) e o ALTER TABLE usa o modo antigo (PRAGMA legacy_alter_table = ON); os dois voltam como estavam no fim.",
            ],
        },
    },
    Caso {
        nome: "alterar_eventos_tipo_pk",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Mudar a coluna tipo de eventos: passa a ser chave primária. A tabela será reconstruída.",
            comandos: &[
                "BEGIN",
                "CREATE TABLE \"_novo_eventos\" (\n    \"momento\" TEXT,\n    \"tipo\" TEXT PRIMARY KEY,\n    \"valor\" REAL,\n    \"dados\" TEXT\n)",
                "INSERT INTO \"_novo_eventos\" (rowid, \"momento\", \"tipo\", \"valor\", \"dados\")\n    SELECT rowid, \"momento\", \"tipo\", \"valor\", \"dados\" FROM \"eventos\"",
                "DROP TABLE \"eventos\"",
                "ALTER TABLE \"_novo_eventos\" RENAME TO \"eventos\"",
                "PRAGMA foreign_key_check(\"eventos\")",
                "COMMIT",
            ],
            reconstroi: true,
            avisos: &[
                "A tabela será reconstruída: os dados são copiados para uma tabela nova com a definição alterada, e os índices, gatilhos e visões ligados a ela são recriados.",
                "Há 4 valores repetidos em tipo; a mudança vai falhar enquanto houver repetição.",
                "Durante a reconstrução as chaves estrangeiras ficam desligadas (PRAGMA foreign_keys = OFF) e o ALTER TABLE usa o modo antigo (PRAGMA legacy_alter_table = ON); os dois voltam como estavam no fim.",
            ],
        },
    },
    Caso {
        nome: "alterar_eventos_tipo_sem_tipo",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Mudar a coluna tipo de eventos: tipo TEXT → sem tipo. A tabela será reconstruída.",
            comandos: &[
                "BEGIN",
                "CREATE TABLE \"_novo_eventos\" (\n    \"momento\" TEXT,\n    \"tipo\",\n    \"valor\" REAL,\n    \"dados\" TEXT\n)",
                "INSERT INTO \"_novo_eventos\" (rowid, \"momento\", \"tipo\", \"valor\", \"dados\")\n    SELECT rowid, \"momento\", \"tipo\", \"valor\", \"dados\" FROM \"eventos\"",
                "DROP TABLE \"eventos\"",
                "ALTER TABLE \"_novo_eventos\" RENAME TO \"eventos\"",
                "PRAGMA foreign_key_check(\"eventos\")",
                "COMMIT",
            ],
            reconstroi: true,
            avisos: &[
                "A tabela será reconstruída: os dados são copiados para uma tabela nova com a definição alterada, e os índices, gatilhos e visões ligados a ela são recriados.",
                "Os valores são copiados e convertidos pela afinidade do tipo novo (BLOB).",
                "Durante a reconstrução as chaves estrangeiras ficam desligadas (PRAGMA foreign_keys = OFF) e o ALTER TABLE usa o modo antigo (PRAGMA legacy_alter_table = ON); os dois voltam como estavam no fim.",
            ],
        },
    },
    Caso {
        nome: "alterar_coluna_aspas_varchar",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Mudar a coluna coluna \"citada\" de Tabela com espaço: tipo TEXT → VARCHAR(50). A tabela será reconstruída.",
            comandos: &[
                "BEGIN",
                "CREATE TABLE \"_novo_Tabela com espaço\" (\n    \"coluna \"\"citada\"\"\" VARCHAR(50),\n    \"número\" INTEGER\n)",
                "INSERT INTO \"_novo_Tabela com espaço\" (rowid, \"coluna \"\"citada\"\"\", \"número\")\n    SELECT rowid, \"coluna \"\"citada\"\"\", \"número\" FROM \"Tabela com espaço\"",
                "DROP TABLE \"Tabela com espaço\"",
                "ALTER TABLE \"_novo_Tabela com espaço\" RENAME TO \"Tabela com espaço\"",
                "PRAGMA foreign_key_check(\"Tabela com espaço\")",
                "COMMIT",
            ],
            reconstroi: true,
            avisos: &[
                "A tabela será reconstruída: os dados são copiados para uma tabela nova com a definição alterada, e os índices, gatilhos e visões ligados a ela são recriados.",
                "Durante a reconstrução as chaves estrangeiras ficam desligadas (PRAGMA foreign_keys = OFF) e o ALTER TABLE usa o modo antigo (PRAGMA legacy_alter_table = ON); os dois voltam como estavam no fim.",
            ],
        },
    },
    Caso {
        nome: "alterar_marcas_sigla_sem_colacao",
        setup: "\nCREATE TABLE marcas (id INTEGER PRIMARY KEY, sigla TEXT UNIQUE COLLATE NOCASE);\nCREATE TABLE modelos (id INTEGER PRIMARY KEY, sigla TEXT REFERENCES marcas (sigla));\nINSERT INTO marcas VALUES (1, 'ABC');\nINSERT INTO modelos VALUES (1, 'abc');\n",
        esperado: Esperado::Plano {
            descricao: "Mudar a coluna sigla de marcas: colação NOCASE → nenhuma. A tabela será reconstruída.",
            comandos: &[
                "BEGIN",
                "CREATE TABLE \"_novo_marcas\" (\n    \"id\" INTEGER PRIMARY KEY,\n    \"sigla\" TEXT UNIQUE\n)",
                "INSERT INTO \"_novo_marcas\" (\"id\", \"sigla\")\n    SELECT \"id\", \"sigla\" FROM \"marcas\"",
                "DROP TABLE \"marcas\"",
                "ALTER TABLE \"_novo_marcas\" RENAME TO \"marcas\"",
                "PRAGMA foreign_key_check(\"marcas\")",
                "PRAGMA foreign_key_check(\"modelos\")",
                "COMMIT",
            ],
            reconstroi: true,
            avisos: &[
                "A tabela será reconstruída: os dados são copiados para uma tabela nova com a definição alterada, e os índices, gatilhos e visões ligados a ela são recriados.",
                "Durante a reconstrução as chaves estrangeiras ficam desligadas (PRAGMA foreign_keys = OFF) e o ALTER TABLE usa o modo antigo (PRAGMA legacy_alter_table = ON); os dois voltam como estavam no fim.",
            ],
        },
    },
    Caso {
        nome: "alterar_com_orfa",
        setup: "\nPRAGMA foreign_keys = OFF;\nINSERT INTO itens_pedido VALUES (99999, 'P001', 1, 1.0);\nPRAGMA foreign_keys = ON;\n",
        esperado: Esperado::Plano {
            descricao: "Mudar a coluna observacao de pedidos: tipo TEXT → VARCHAR(10). A tabela será reconstruída.",
            comandos: &[
                "BEGIN",
                "CREATE TABLE \"_novo_pedidos\" (\n    \"id\" INTEGER PRIMARY KEY,\n    \"cliente_id\" INTEGER NOT NULL REFERENCES \"clientes\" (\"id\") ON DELETE CASCADE,\n    \"feito_em\" TEXT NOT NULL,\n    \"status\" TEXT NOT NULL CHECK (status IN ('aberto', 'pago', 'cancelado')) DEFAULT 'aberto',\n    \"observacao\" VARCHAR(10)\n)",
                "INSERT INTO \"_novo_pedidos\" (\"id\", \"cliente_id\", \"feito_em\", \"status\", \"observacao\")\n    SELECT \"id\", \"cliente_id\", \"feito_em\", \"status\", \"observacao\" FROM \"pedidos\"",
                "DROP VIEW \"v_pedidos_por_cliente\"",
                "DROP TABLE \"pedidos\"",
                "ALTER TABLE \"_novo_pedidos\" RENAME TO \"pedidos\"",
                "CREATE INDEX idx_pedidos_cliente ON pedidos (cliente_id)",
                "CREATE INDEX idx_pedidos_status_data ON pedidos (status, feito_em DESC)",
                "CREATE TRIGGER trg_pedido_cancelado AFTER UPDATE OF status ON pedidos\n    WHEN new.status = 'cancelado'\nBEGIN\n    UPDATE pedidos SET observacao = coalesce(observacao || ' · ', '') || 'cancelado' WHERE id = new.id;\nEND",
                "CREATE VIEW v_pedidos_por_cliente AS\n    SELECT c.id, c.nome, count(p.id) AS pedidos, max(p.feito_em) AS ultimo\n    FROM clientes c LEFT JOIN pedidos p ON p.cliente_id = c.id\n    GROUP BY c.id",
                "PRAGMA foreign_key_check(\"pedidos\")",
                "PRAGMA foreign_key_check(\"itens_pedido\")",
                "COMMIT",
            ],
            reconstroi: true,
            avisos: &[
                "A tabela será reconstruída: os dados são copiados para uma tabela nova com a definição alterada, e os índices, gatilhos e visões ligados a ela são recriados.",
                "O banco já tem 1 linha de itens_pedido apontando para linhas que não existem em pedidos. Elas ficam como estão: a verificação no fim da reconstrução só recusa problemas novos.",
                "Durante a reconstrução as chaves estrangeiras ficam desligadas (PRAGMA foreign_keys = OFF) e o ALTER TABLE usa o modo antigo (PRAGMA legacy_alter_table = ON); os dois voltam como estavam no fim.",
            ],
        },
    },
    Caso {
        nome: "alterar_registro_nota",
        setup: "\nCREATE TABLE categoria (id INTEGER PRIMARY KEY, nome TEXT);\nCREATE TABLE registro (\n  id INTEGER CONSTRAINT pk_registro PRIMARY KEY -- chave\n      AUTOINCREMENT,\n  categoria_id INTEGER CONSTRAINT fk_cat REFERENCES categoria (id) -- dono\n      ON DELETE CASCADE,\n  nota TEXT\n);\nINSERT INTO categoria VALUES (1, 'Raiz'), (2, 'Outra');\nINSERT INTO registro (categoria_id, nota) VALUES (1, 'a'), (1, 'b'), (2, 'c');\nDELETE FROM registro WHERE id = 3;\n",
        esperado: Esperado::Plano {
            descricao: "Mudar a coluna nota de registro: tipo TEXT → VARCHAR(80). A tabela será reconstruída.",
            comandos: &[
                "BEGIN",
                "CREATE TABLE \"_novo_registro\" (\n    \"id\" INTEGER CONSTRAINT pk_registro PRIMARY KEY /* chave */ AUTOINCREMENT,\n    \"categoria_id\" INTEGER CONSTRAINT fk_cat REFERENCES categoria (id) /* dono */ ON DELETE CASCADE,\n    \"nota\" VARCHAR(80)\n)",
                "INSERT INTO \"_novo_registro\" (\"id\", \"categoria_id\", \"nota\")\n    SELECT \"id\", \"categoria_id\", \"nota\" FROM \"registro\"",
                "DELETE FROM sqlite_sequence WHERE name = '_novo_registro'",
                "INSERT INTO sqlite_sequence (name, seq) SELECT '_novo_registro', seq FROM sqlite_sequence WHERE name = 'registro'",
                "DROP TABLE \"registro\"",
                "ALTER TABLE \"_novo_registro\" RENAME TO \"registro\"",
                "PRAGMA foreign_key_check(\"registro\")",
                "COMMIT",
            ],
            reconstroi: true,
            avisos: &[
                "A tabela será reconstruída: os dados são copiados para uma tabela nova com a definição alterada, e os índices, gatilhos e visões ligados a ela são recriados.",
                "Durante a reconstrução as chaves estrangeiras ficam desligadas (PRAGMA foreign_keys = OFF) e o ALTER TABLE usa o modo antigo (PRAGMA legacy_alter_table = ON); os dois voltam como estavam no fim.",
            ],
        },
    },
    Caso {
        nome: "alterar_t_desc_v",
        setup: "\nCREATE TABLE t_desc (id INTEGER PRIMARY KEY DESC, v TEXT);\nCREATE TABLE t_comp (a INTEGER, b TEXT, PRIMARY KEY (a));\nCREATE TABLE t_oid (rowid TEXT, oid TEXT, v TEXT);\nCREATE TABLE t_unica (so TEXT);\nINSERT INTO t_desc VALUES (5, 'x'); INSERT INTO t_comp VALUES (1, 'b'); INSERT INTO t_oid VALUES ('r', 'o', 'v');\n",
        esperado: Esperado::Plano {
            descricao: "Mudar a coluna v de t_desc: tipo TEXT → VARCHAR(9). A tabela será reconstruída.",
            comandos: &[
                "BEGIN",
                "CREATE TABLE \"_novo_t_desc\" (\n    \"id\" INTEGER PRIMARY KEY DESC,\n    \"v\" VARCHAR(9)\n)",
                "INSERT INTO \"_novo_t_desc\" (rowid, \"id\", \"v\")\n    SELECT rowid, \"id\", \"v\" FROM \"t_desc\"",
                "DROP TABLE \"t_desc\"",
                "ALTER TABLE \"_novo_t_desc\" RENAME TO \"t_desc\"",
                "PRAGMA foreign_key_check(\"t_desc\")",
                "COMMIT",
            ],
            reconstroi: true,
            avisos: &[
                "A tabela será reconstruída: os dados são copiados para uma tabela nova com a definição alterada, e os índices, gatilhos e visões ligados a ela são recriados.",
                "Durante a reconstrução as chaves estrangeiras ficam desligadas (PRAGMA foreign_keys = OFF) e o ALTER TABLE usa o modo antigo (PRAGMA legacy_alter_table = ON); os dois voltam como estavam no fim.",
            ],
        },
    },
    Caso {
        nome: "alterar_t_comp_b",
        setup: "\nCREATE TABLE t_desc (id INTEGER PRIMARY KEY DESC, v TEXT);\nCREATE TABLE t_comp (a INTEGER, b TEXT, PRIMARY KEY (a));\nCREATE TABLE t_oid (rowid TEXT, oid TEXT, v TEXT);\nCREATE TABLE t_unica (so TEXT);\nINSERT INTO t_desc VALUES (5, 'x'); INSERT INTO t_comp VALUES (1, 'b'); INSERT INTO t_oid VALUES ('r', 'o', 'v');\n",
        esperado: Esperado::Plano {
            descricao: "Mudar a coluna b de t_comp: tipo TEXT → VARCHAR(9). A tabela será reconstruída.",
            comandos: &[
                "BEGIN",
                "CREATE TABLE \"_novo_t_comp\" (\n    \"a\" INTEGER,\n    \"b\" VARCHAR(9),\n    PRIMARY KEY (a)\n)",
                "INSERT INTO \"_novo_t_comp\" (\"a\", \"b\")\n    SELECT \"a\", \"b\" FROM \"t_comp\"",
                "DROP TABLE \"t_comp\"",
                "ALTER TABLE \"_novo_t_comp\" RENAME TO \"t_comp\"",
                "PRAGMA foreign_key_check(\"t_comp\")",
                "COMMIT",
            ],
            reconstroi: true,
            avisos: &[
                "A tabela será reconstruída: os dados são copiados para uma tabela nova com a definição alterada, e os índices, gatilhos e visões ligados a ela são recriados.",
                "Durante a reconstrução as chaves estrangeiras ficam desligadas (PRAGMA foreign_keys = OFF) e o ALTER TABLE usa o modo antigo (PRAGMA legacy_alter_table = ON); os dois voltam como estavam no fim.",
            ],
        },
    },
    Caso {
        nome: "alterar_t_oid_v",
        setup: "\nCREATE TABLE t_desc (id INTEGER PRIMARY KEY DESC, v TEXT);\nCREATE TABLE t_comp (a INTEGER, b TEXT, PRIMARY KEY (a));\nCREATE TABLE t_oid (rowid TEXT, oid TEXT, v TEXT);\nCREATE TABLE t_unica (so TEXT);\nINSERT INTO t_desc VALUES (5, 'x'); INSERT INTO t_comp VALUES (1, 'b'); INSERT INTO t_oid VALUES ('r', 'o', 'v');\n",
        esperado: Esperado::Plano {
            descricao: "Mudar a coluna v de t_oid: tipo TEXT → VARCHAR(9). A tabela será reconstruída.",
            comandos: &[
                "BEGIN",
                "CREATE TABLE \"_novo_t_oid\" (\n    \"rowid\" TEXT,\n    \"oid\" TEXT,\n    \"v\" VARCHAR(9)\n)",
                "INSERT INTO \"_novo_t_oid\" (_rowid_, \"rowid\", \"oid\", \"v\")\n    SELECT _rowid_, \"rowid\", \"oid\", \"v\" FROM \"t_oid\"",
                "DROP TABLE \"t_oid\"",
                "ALTER TABLE \"_novo_t_oid\" RENAME TO \"t_oid\"",
                "PRAGMA foreign_key_check(\"t_oid\")",
                "COMMIT",
            ],
            reconstroi: true,
            avisos: &[
                "A tabela será reconstruída: os dados são copiados para uma tabela nova com a definição alterada, e os índices, gatilhos e visões ligados a ela são recriados.",
                "Durante a reconstrução as chaves estrangeiras ficam desligadas (PRAGMA foreign_keys = OFF) e o ALTER TABLE usa o modo antigo (PRAGMA legacy_alter_table = ON); os dois voltam como estavam no fim.",
            ],
        },
    },
    Caso {
        nome: "alterar_auditoria_mensagem",
        setup: "\nCREATE TABLE auditoria (id INTEGER PRIMARY KEY, pedido INTEGER, mensagem TEXT, visto INTEGER);\nCREATE TRIGGER tg_pedidos_auditoria AFTER INSERT ON pedidos BEGIN\n    INSERT INTO auditoria (pedido, mensagem) VALUES (new.id, 'criado');\nEND;\nCREATE TRIGGER tg_pedidos_visto AFTER UPDATE ON pedidos BEGIN\n    UPDATE auditoria SET visto = 1 WHERE pedido = new.id;\nEND;\nCREATE VIEW v_tipos AS SELECT tipo, count(*) AS n FROM eventos GROUP BY tipo;\nCREATE VIEW v_auditoria AS SELECT * FROM auditoria;\nCREATE TRIGGER tg_v_auditoria INSTEAD OF INSERT ON v_auditoria BEGIN SELECT 1; END;\n",
        esperado: Esperado::Plano {
            descricao: "Mudar a coluna mensagem de auditoria: tipo TEXT → VARCHAR(9). A tabela será reconstruída.",
            comandos: &[
                "BEGIN",
                "CREATE TABLE \"_novo_auditoria\" (\n    \"id\" INTEGER PRIMARY KEY,\n    \"pedido\" INTEGER,\n    \"mensagem\" VARCHAR(9),\n    \"visto\" INTEGER\n)",
                "INSERT INTO \"_novo_auditoria\" (\"id\", \"pedido\", \"mensagem\", \"visto\")\n    SELECT \"id\", \"pedido\", \"mensagem\", \"visto\" FROM \"auditoria\"",
                "DROP VIEW \"v_auditoria\"",
                "DROP TABLE \"auditoria\"",
                "ALTER TABLE \"_novo_auditoria\" RENAME TO \"auditoria\"",
                "CREATE VIEW v_auditoria AS SELECT * FROM auditoria",
                "CREATE TRIGGER tg_v_auditoria INSTEAD OF INSERT ON v_auditoria BEGIN SELECT 1; END",
                "PRAGMA foreign_key_check(\"auditoria\")",
                "COMMIT",
            ],
            reconstroi: true,
            avisos: &[
                "A tabela será reconstruída: os dados são copiados para uma tabela nova com a definição alterada, e os índices, gatilhos e visões ligados a ela são recriados.",
                "Durante a reconstrução as chaves estrangeiras ficam desligadas (PRAGMA foreign_keys = OFF) e o ALTER TABLE usa o modo antigo (PRAGMA legacy_alter_table = ON); os dois voltam como estavam no fim.",
            ],
        },
    },
    Caso {
        nome: "alterar_erro_coluna_existe",
        setup: "",
        esperado: Esperado::Erro { mensagem: "A tabela clientes já tem uma coluna chamada EMAIL.", status: 409 },
    },
    Caso {
        nome: "alterar_erro_auto",
        setup: "",
        esperado: Esperado::Erro { mensagem: "Autoincremento só vale numa chave primária INTEGER.", status: 400 },
    },
    Caso {
        nome: "alterar_erro_nome_vazio",
        setup: "",
        esperado: Esperado::Erro { mensagem: "Toda coluna precisa de um nome.", status: 400 },
    },
    Caso {
        nome: "alterar_erro_coluna_nao_existe",
        setup: "",
        esperado: Esperado::Erro { mensagem: "A coluna xyz não existe em clientes.", status: 404 },
    },
    Caso {
        nome: "alterar_erro_tabela_nao_existe",
        setup: "",
        esperado: Esperado::Erro { mensagem: "A tabela xyz não existe.", status: 404 },
    },
    Caso {
        nome: "alterar_erro_tabela_vazia",
        setup: "",
        esperado: Esperado::Erro { mensagem: "Informe o nome da tabela.", status: 400 },
    },
    Caso {
        nome: "alterar_erro_coluna_vazia",
        setup: "",
        esperado: Esperado::Erro { mensagem: "Informe o nome da coluna.", status: 400 },
    },
    Caso {
        nome: "alterar_erro_tipo_invalido",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "O tipo «TEXT NOT NULL» não pode ter restrições (NOT); use os campos da coluna para isso.",
            status: 400,
        },
    },
    Caso {
        nome: "alterar_erro_acao_invalida",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "Ação inválida em Ao excluir: APAGAR. Use uma destas: SET NULL, SET DEFAULT, CASCADE, RESTRICT, NO ACTION.",
            status: 400,
        },
    },
    Caso {
        nome: "alterar_erro_definicao_invalida",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "A regra de verificação tem parênteses sem fechamento: cidade <> 'x' (",
            status: 400,
        },
    },
    Caso {
        nome: "alterar_erro_tabela_shadow",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "notas_data é uma tabela auxiliar de uma tabela virtual; mudar a estrutura dela estragaria a tabela virtual.",
            status: 403,
        },
    },
    Caso {
        nome: "executar_limite_nao_nulo",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "Não foi possível aplicar a mudança; nada foi alterado. A coluna limite não aceita valor nulo.",
            status: 400,
        },
    },
    Caso {
        nome: "executar_marcas_sigla",
        setup: "\nCREATE TABLE marcas (id INTEGER PRIMARY KEY, sigla TEXT UNIQUE COLLATE NOCASE);\nCREATE TABLE modelos (id INTEGER PRIMARY KEY, sigla TEXT REFERENCES marcas (sigla));\nINSERT INTO marcas VALUES (1, 'ABC');\nINSERT INTO modelos VALUES (1, 'abc');\n",
        esperado: Esperado::Erro {
            mensagem: "A mudança deixaria 1 linha de modelos apontando para linhas que não existem em marcas. Nada foi alterado.",
            status: 400,
        },
    },
    Caso {
        nome: "executar_transacao_aberta",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "Há uma transação aberta (na Consulta SQL). Grave com COMMIT ou descarte com ROLLBACK antes de mudar a estrutura.",
            status: 409,
        },
    },
    Caso { nome: "executar_clientes_cidade", setup: "", esperado: Esperado::Feito },
    Caso {
        nome: "executar_excluir_editoras",
        setup: "CREATE TABLE editoras (id INTEGER PRIMARY KEY); CREATE TABLE livros (id INTEGER PRIMARY KEY, editora_id INTEGER REFERENCES editoras (id)); INSERT INTO editoras VALUES (1); INSERT INTO livros VALUES (1, 1), (2, 1);",
        esperado: Esperado::Erro {
            mensagem: "Não foi possível aplicar a mudança; nada foi alterado. A alteração quebra uma chave estrangeira: o valor não existe na tabela referenciada, ou outra tabela ainda aponta para esta linha.",
            status: 400,
        },
    },
    Caso {
        nome: "adicionar_eventos_peso",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Adicionar a coluna peso (INTEGER) em eventos.",
            comandos: &["ALTER TABLE \"eventos\" ADD COLUMN \"peso\" INTEGER NOT NULL DEFAULT 0"],
            reconstroi: false,
            avisos: &["As linhas que já existem recebem o valor padrão 0."],
        },
    },
    Caso {
        nome: "adicionar_eventos_obs",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Adicionar a coluna obs (TEXT) em eventos.",
            comandos: &["ALTER TABLE \"eventos\" ADD COLUMN \"obs\" TEXT DEFAULT 'nenhuma'"],
            reconstroi: false,
            avisos: &["As linhas que já existem recebem o valor padrão 'nenhuma'."],
        },
    },
    Caso {
        nome: "adicionar_eventos_sem_tipo",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Adicionar a coluna x (sem tipo) em eventos.",
            comandos: &["ALTER TABLE \"eventos\" ADD COLUMN \"x\""],
            reconstroi: false,
            avisos: &[],
        },
    },
    Caso {
        nome: "adicionar_clientes_documento",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Adicionar a coluna documento (TEXT) em clientes: coluna sem repetição (UNIQUE) só entra reconstruindo a tabela. A tabela será reconstruída.",
            comandos: &[
                "BEGIN",
                "CREATE TABLE \"_novo_clientes\" (\n    \"id\" INTEGER PRIMARY KEY AUTOINCREMENT,\n    \"nome\" TEXT NOT NULL,\n    \"email\" TEXT UNIQUE,\n    \"cidade\" TEXT DEFAULT 'São Paulo',\n    \"limite\" REAL CHECK (limite >= 0),\n    \"ativo\" INTEGER NOT NULL DEFAULT 1,\n    \"criado_em\" TEXT DEFAULT CURRENT_TIMESTAMP,\n    \"documento\" TEXT UNIQUE\n)",
                "INSERT INTO \"_novo_clientes\" (\"id\", \"nome\", \"email\", \"cidade\", \"limite\", \"ativo\", \"criado_em\")\n    SELECT \"id\", \"nome\", \"email\", \"cidade\", \"limite\", \"ativo\", \"criado_em\" FROM \"clientes\"",
                "DELETE FROM sqlite_sequence WHERE name = '_novo_clientes'",
                "INSERT INTO sqlite_sequence (name, seq) SELECT '_novo_clientes', seq FROM sqlite_sequence WHERE name = 'clientes'",
                "DROP VIEW \"v_pedidos_por_cliente\"",
                "DROP TABLE \"clientes\"",
                "ALTER TABLE \"_novo_clientes\" RENAME TO \"clientes\"",
                "CREATE UNIQUE INDEX idx_clientes_nome_cidade ON clientes (nome, cidade) WHERE ativo = 1",
                "CREATE VIEW v_pedidos_por_cliente AS\n    SELECT c.id, c.nome, count(p.id) AS pedidos, max(p.feito_em) AS ultimo\n    FROM clientes c LEFT JOIN pedidos p ON p.cliente_id = c.id\n    GROUP BY c.id",
                "PRAGMA foreign_key_check(\"clientes\")",
                "PRAGMA foreign_key_check(\"pedidos\")",
                "COMMIT",
            ],
            reconstroi: true,
            avisos: &[
                "A tabela será reconstruída: os dados são copiados para uma tabela nova com a definição alterada, e os índices, gatilhos e visões ligados a ela são recriados.",
                "Durante a reconstrução as chaves estrangeiras ficam desligadas (PRAGMA foreign_keys = OFF) e o ALTER TABLE usa o modo antigo (PRAGMA legacy_alter_table = ON); os dois voltam como estavam no fim.",
            ],
        },
    },
    Caso {
        nome: "adicionar_eventos_registrado",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Adicionar a coluna registrado_em (TEXT) em eventos: valor padrão que não é constante só entra reconstruindo a tabela. A tabela será reconstruída.",
            comandos: &[
                "BEGIN",
                "CREATE TABLE \"_novo_eventos\" (\n    \"momento\" TEXT,\n    \"tipo\" TEXT,\n    \"valor\" REAL,\n    \"dados\" TEXT,\n    \"registrado_em\" TEXT DEFAULT CURRENT_TIMESTAMP\n)",
                "INSERT INTO \"_novo_eventos\" (rowid, \"momento\", \"tipo\", \"valor\", \"dados\")\n    SELECT rowid, \"momento\", \"tipo\", \"valor\", \"dados\" FROM \"eventos\"",
                "DROP TABLE \"eventos\"",
                "ALTER TABLE \"_novo_eventos\" RENAME TO \"eventos\"",
                "PRAGMA foreign_key_check(\"eventos\")",
                "COMMIT",
            ],
            reconstroi: true,
            avisos: &[
                "A tabela será reconstruída: os dados são copiados para uma tabela nova com a definição alterada, e os índices, gatilhos e visões ligados a ela são recriados.",
                "Durante a reconstrução as chaves estrangeiras ficam desligadas (PRAGMA foreign_keys = OFF) e o ALTER TABLE usa o modo antigo (PRAGMA legacy_alter_table = ON); os dois voltam como estavam no fim.",
            ],
        },
    },
    Caso {
        nome: "adicionar_eventos_expressao",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Adicionar a coluna dia (TEXT) em eventos: valor padrão que não é constante só entra reconstruindo a tabela. A tabela será reconstruída.",
            comandos: &[
                "BEGIN",
                "CREATE TABLE \"_novo_eventos\" (\n    \"momento\" TEXT,\n    \"tipo\" TEXT,\n    \"valor\" REAL,\n    \"dados\" TEXT,\n    \"dia\" TEXT DEFAULT (date('now'))\n)",
                "INSERT INTO \"_novo_eventos\" (rowid, \"momento\", \"tipo\", \"valor\", \"dados\")\n    SELECT rowid, \"momento\", \"tipo\", \"valor\", \"dados\" FROM \"eventos\"",
                "DROP TABLE \"eventos\"",
                "ALTER TABLE \"_novo_eventos\" RENAME TO \"eventos\"",
                "PRAGMA foreign_key_check(\"eventos\")",
                "COMMIT",
            ],
            reconstroi: true,
            avisos: &[
                "A tabela será reconstruída: os dados são copiados para uma tabela nova com a definição alterada, e os índices, gatilhos e visões ligados a ela são recriados.",
                "Durante a reconstrução as chaves estrangeiras ficam desligadas (PRAGMA foreign_keys = OFF) e o ALTER TABLE usa o modo antigo (PRAGMA legacy_alter_table = ON); os dois voltam como estavam no fim.",
            ],
        },
    },
    Caso {
        nome: "adicionar_eventos_pk",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Adicionar a coluna codigo (INTEGER) em eventos: chave primária só entra reconstruindo a tabela. A tabela será reconstruída.",
            comandos: &[
                "BEGIN",
                "CREATE TABLE \"_novo_eventos\" (\n    \"momento\" TEXT,\n    \"tipo\" TEXT,\n    \"valor\" REAL,\n    \"dados\" TEXT,\n    \"codigo\" INTEGER PRIMARY KEY\n)",
                "INSERT INTO \"_novo_eventos\" (\"momento\", \"tipo\", \"valor\", \"dados\")\n    SELECT \"momento\", \"tipo\", \"valor\", \"dados\" FROM \"eventos\"",
                "DROP TABLE \"eventos\"",
                "ALTER TABLE \"_novo_eventos\" RENAME TO \"eventos\"",
                "PRAGMA foreign_key_check(\"eventos\")",
                "COMMIT",
            ],
            reconstroi: true,
            avisos: &[
                "A tabela será reconstruída: os dados são copiados para uma tabela nova com a definição alterada, e os índices, gatilhos e visões ligados a ela são recriados.",
                "Durante a reconstrução as chaves estrangeiras ficam desligadas (PRAGMA foreign_keys = OFF) e o ALTER TABLE usa o modo antigo (PRAGMA legacy_alter_table = ON); os dois voltam como estavam no fim.",
            ],
        },
    },
    Caso {
        nome: "adicionar_eventos_stored",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Adicionar a coluna tam (INTEGER) em eventos: coluna gerada armazenada (STORED) só entra reconstruindo a tabela. A tabela será reconstruída.",
            comandos: &[
                "BEGIN",
                "CREATE TABLE \"_novo_eventos\" (\n    \"momento\" TEXT,\n    \"tipo\" TEXT,\n    \"valor\" REAL,\n    \"dados\" TEXT,\n    \"tam\" INTEGER GENERATED ALWAYS AS (length(dados)) STORED\n)",
                "INSERT INTO \"_novo_eventos\" (rowid, \"momento\", \"tipo\", \"valor\", \"dados\")\n    SELECT rowid, \"momento\", \"tipo\", \"valor\", \"dados\" FROM \"eventos\"",
                "DROP TABLE \"eventos\"",
                "ALTER TABLE \"_novo_eventos\" RENAME TO \"eventos\"",
                "PRAGMA foreign_key_check(\"eventos\")",
                "COMMIT",
            ],
            reconstroi: true,
            avisos: &[
                "A tabela será reconstruída: os dados são copiados para uma tabela nova com a definição alterada, e os índices, gatilhos e visões ligados a ela são recriados.",
                "Durante a reconstrução as chaves estrangeiras ficam desligadas (PRAGMA foreign_keys = OFF) e o ALTER TABLE usa o modo antigo (PRAGMA legacy_alter_table = ON); os dois voltam como estavam no fim.",
            ],
        },
    },
    Caso {
        nome: "adicionar_eventos_virtual",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Adicionar a coluna tam (INTEGER) em eventos.",
            comandos: &[
                "ALTER TABLE \"eventos\" ADD COLUMN \"tam\" INTEGER GENERATED ALWAYS AS (length(dados)) VIRTUAL",
            ],
            reconstroi: false,
            avisos: &[],
        },
    },
    Caso {
        nome: "adicionar_pedidos_fk_padrao",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Adicionar a coluna outro (INTEGER) em pedidos: chave estrangeira com valor padrão só entra reconstruindo a tabela. A tabela será reconstruída.",
            comandos: &[
                "BEGIN",
                "CREATE TABLE \"_novo_pedidos\" (\n    \"id\" INTEGER PRIMARY KEY,\n    \"cliente_id\" INTEGER NOT NULL REFERENCES \"clientes\" (\"id\") ON DELETE CASCADE,\n    \"feito_em\" TEXT NOT NULL,\n    \"status\" TEXT NOT NULL CHECK (status IN ('aberto', 'pago', 'cancelado')) DEFAULT 'aberto',\n    \"observacao\" TEXT,\n    \"outro\" INTEGER DEFAULT 1 REFERENCES \"clientes\" (\"id\")\n)",
                "INSERT INTO \"_novo_pedidos\" (\"id\", \"cliente_id\", \"feito_em\", \"status\", \"observacao\")\n    SELECT \"id\", \"cliente_id\", \"feito_em\", \"status\", \"observacao\" FROM \"pedidos\"",
                "DROP VIEW \"v_pedidos_por_cliente\"",
                "DROP TABLE \"pedidos\"",
                "ALTER TABLE \"_novo_pedidos\" RENAME TO \"pedidos\"",
                "CREATE INDEX idx_pedidos_cliente ON pedidos (cliente_id)",
                "CREATE INDEX idx_pedidos_status_data ON pedidos (status, feito_em DESC)",
                "CREATE TRIGGER trg_pedido_cancelado AFTER UPDATE OF status ON pedidos\n    WHEN new.status = 'cancelado'\nBEGIN\n    UPDATE pedidos SET observacao = coalesce(observacao || ' · ', '') || 'cancelado' WHERE id = new.id;\nEND",
                "CREATE VIEW v_pedidos_por_cliente AS\n    SELECT c.id, c.nome, count(p.id) AS pedidos, max(p.feito_em) AS ultimo\n    FROM clientes c LEFT JOIN pedidos p ON p.cliente_id = c.id\n    GROUP BY c.id",
                "PRAGMA foreign_key_check(\"pedidos\")",
                "PRAGMA foreign_key_check(\"itens_pedido\")",
                "COMMIT",
            ],
            reconstroi: true,
            avisos: &[
                "A tabela será reconstruída: os dados são copiados para uma tabela nova com a definição alterada, e os índices, gatilhos e visões ligados a ela são recriados.",
                "Durante a reconstrução as chaves estrangeiras ficam desligadas (PRAGMA foreign_keys = OFF) e o ALTER TABLE usa o modo antigo (PRAGMA legacy_alter_table = ON); os dois voltam como estavam no fim.",
            ],
        },
    },
    Caso {
        nome: "adicionar_pedidos_fk",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Adicionar a coluna outro (INTEGER) em pedidos.",
            comandos: &[
                "ALTER TABLE \"pedidos\" ADD COLUMN \"outro\" INTEGER REFERENCES \"clientes\" (\"id\") ON DELETE CASCADE",
            ],
            reconstroi: false,
            avisos: &[],
        },
    },
    Caso {
        nome: "adicionar_vazia_nao_nulo",
        setup: "CREATE TABLE vazia (a TEXT);",
        esperado: Esperado::Plano {
            descricao: "Adicionar a coluna b (TEXT) em vazia: coluna sem nulo e sem valor padrão só entra reconstruindo a tabela. A tabela será reconstruída.",
            comandos: &[
                "BEGIN",
                "CREATE TABLE \"_novo_vazia\" (\n    \"a\" TEXT,\n    \"b\" TEXT NOT NULL\n)",
                "INSERT INTO \"_novo_vazia\" (rowid, \"a\")\n    SELECT rowid, \"a\" FROM \"vazia\"",
                "DROP TABLE \"vazia\"",
                "ALTER TABLE \"_novo_vazia\" RENAME TO \"vazia\"",
                "PRAGMA foreign_key_check(\"vazia\")",
                "COMMIT",
            ],
            reconstroi: true,
            avisos: &[
                "A tabela será reconstruída: os dados são copiados para uma tabela nova com a definição alterada, e os índices, gatilhos e visões ligados a ela são recriados.",
                "Durante a reconstrução as chaves estrangeiras ficam desligadas (PRAGMA foreign_keys = OFF) e o ALTER TABLE usa o modo antigo (PRAGMA legacy_alter_table = ON); os dois voltam como estavam no fim.",
            ],
        },
    },
    Caso {
        nome: "adicionar_espaco_unica",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Adicionar a coluna única [sim] (INTEGER) em Tabela com espaço: coluna sem repetição (UNIQUE) só entra reconstruindo a tabela. A tabela será reconstruída.",
            comandos: &[
                "BEGIN",
                "CREATE TABLE \"_novo_Tabela com espaço\" (\n    \"coluna \"\"citada\"\"\" TEXT,\n    \"número\" INTEGER,\n    \"única [sim]\" INTEGER UNIQUE\n)",
                "INSERT INTO \"_novo_Tabela com espaço\" (rowid, \"coluna \"\"citada\"\"\", \"número\")\n    SELECT rowid, \"coluna \"\"citada\"\"\", \"número\" FROM \"Tabela com espaço\"",
                "DROP TABLE \"Tabela com espaço\"",
                "ALTER TABLE \"_novo_Tabela com espaço\" RENAME TO \"Tabela com espaço\"",
                "PRAGMA foreign_key_check(\"Tabela com espaço\")",
                "COMMIT",
            ],
            reconstroi: true,
            avisos: &[
                "A tabela será reconstruída: os dados são copiados para uma tabela nova com a definição alterada, e os índices, gatilhos e visões ligados a ela são recriados.",
                "Durante a reconstrução as chaves estrangeiras ficam desligadas (PRAGMA foreign_keys = OFF) e o ALTER TABLE usa o modo antigo (PRAGMA legacy_alter_table = ON); os dois voltam como estavam no fim.",
            ],
        },
    },
    Caso {
        nome: "adicionar_erro_nao_nulo",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "A coluna x não aceita nulo, então precisa de um valor padrão: a tabela já tem linhas, e elas ficariam sem valor.",
            status: 400,
        },
    },
    Caso {
        nome: "adicionar_erro_existe",
        setup: "",
        esperado: Esperado::Erro { mensagem: "A tabela eventos já tem uma coluna chamada TIPO.", status: 409 },
    },
    Caso {
        nome: "adicionar_erro_auto",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "Autoincremento só pode ser definido ao criar a tabela, na chave primária INTEGER.",
            status: 400,
        },
    },
    Caso {
        nome: "adicionar_erro_sem_nome",
        setup: "",
        esperado: Esperado::Erro { mensagem: "Toda coluna precisa de um nome.", status: 400 },
    },
    Caso {
        nome: "adicionar_erro_visao",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "v_pedidos_por_cliente é uma visão: a estrutura dela é o próprio SELECT. Para mudá-la, exclua a visão e crie de novo.",
            status: 400,
        },
    },
    Caso {
        nome: "adicionar_erro_virtual",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "notas é uma tabela virtual: as colunas vêm do módulo que a criou e não podem ser editadas.",
            status: 400,
        },
    },
    Caso {
        nome: "adicionar_erro_interna",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "sqlite_sequence é uma tabela interna do SQLite; a estrutura dela não pode ser mudada.",
            status: 403,
        },
    },
    Caso {
        nome: "adicionar_erro_padrao",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "O valor padrão não pode ter ponto e vírgula: 1; DROP TABLE x",
            status: 400,
        },
    },
    Caso {
        nome: "excluir_eventos_dados",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Excluir a coluna dados de eventos.",
            comandos: &["ALTER TABLE \"eventos\" DROP COLUMN \"dados\""],
            reconstroi: false,
            avisos: &["Colunas removidas perdem os dados."],
        },
    },
    Caso {
        nome: "excluir_erro_clientes_id",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "A coluna id faz parte da chave primária de clientes e não pode ser excluída.",
            status: 400,
        },
    },
    Caso {
        nome: "excluir_erro_itens_produto",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "A coluna produto faz parte da chave primária de itens_pedido e não pode ser excluída.",
            status: 400,
        },
    },
    Caso {
        nome: "excluir_erro_clientes_email",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "A coluna email não aceita repetição (UNIQUE). Tire essa regra da coluna antes (isso reconstrói a tabela).",
            status: 400,
        },
    },
    Caso {
        nome: "excluir_erro_clientes_nome",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "A coluna nome é usada pelo índice idx_clientes_nome_cidade. Exclua o índice antes.",
            status: 400,
        },
    },
    Caso {
        nome: "excluir_erro_pedidos_feito_em",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "A coluna feito_em é usada pelo índice idx_pedidos_status_data. Exclua o índice antes.",
            status: 400,
        },
    },
    Caso {
        nome: "excluir_erro_produtos_preco",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "A coluna gerada preco_centavos depende de preco. Exclua-a ou mude a expressão dela antes.",
            status: 400,
        },
    },
    Caso {
        nome: "excluir_erro_pedidos_observacao",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "A coluna observacao é usada pelo gatilho trg_pedido_cancelado. Exclua o gatilho antes.",
            status: 400,
        },
    },
    Caso {
        nome: "excluir_erro_pedidos_cliente_id",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "A coluna cliente_id é usada pelo índice idx_pedidos_cliente. Exclua o índice antes.",
            status: 400,
        },
    },
    Caso {
        nome: "excluir_erro_clientes_limite",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Excluir a coluna limite de clientes.",
            comandos: &["ALTER TABLE \"clientes\" DROP COLUMN \"limite\""],
            reconstroi: false,
            avisos: &["Colunas removidas perdem os dados."],
        },
    },
    Caso {
        nome: "excluir_erro_clientes_ativo",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "A coluna ativo é usada pelo índice idx_clientes_nome_cidade. Exclua o índice antes.",
            status: 400,
        },
    },
    Caso {
        nome: "excluir_erro_pedidos_status",
        setup: "DROP INDEX idx_pedidos_status_data;",
        esperado: Esperado::Erro {
            mensagem: "A coluna status é usada pelo gatilho trg_pedido_cancelado. Exclua o gatilho antes.",
            status: 400,
        },
    },
    Caso {
        nome: "excluir_erro_auditoria_mensagem",
        setup: "\nCREATE TABLE auditoria (id INTEGER PRIMARY KEY, pedido INTEGER, mensagem TEXT, visto INTEGER);\nCREATE TRIGGER tg_pedidos_auditoria AFTER INSERT ON pedidos BEGIN\n    INSERT INTO auditoria (pedido, mensagem) VALUES (new.id, 'criado');\nEND;\nCREATE TRIGGER tg_pedidos_visto AFTER UPDATE ON pedidos BEGIN\n    UPDATE auditoria SET visto = 1 WHERE pedido = new.id;\nEND;\nCREATE VIEW v_tipos AS SELECT tipo, count(*) AS n FROM eventos GROUP BY tipo;\nCREATE VIEW v_auditoria AS SELECT * FROM auditoria;\nCREATE TRIGGER tg_v_auditoria INSTEAD OF INSERT ON v_auditoria BEGIN SELECT 1; END;\n",
        esperado: Esperado::Erro {
            mensagem: "A coluna mensagem é usada pelo gatilho tg_pedidos_auditoria (de pedidos). Exclua o gatilho antes.",
            status: 400,
        },
    },
    Caso {
        nome: "excluir_erro_auditoria_visto",
        setup: "\nCREATE TABLE auditoria (id INTEGER PRIMARY KEY, pedido INTEGER, mensagem TEXT, visto INTEGER);\nCREATE TRIGGER tg_pedidos_auditoria AFTER INSERT ON pedidos BEGIN\n    INSERT INTO auditoria (pedido, mensagem) VALUES (new.id, 'criado');\nEND;\nCREATE TRIGGER tg_pedidos_visto AFTER UPDATE ON pedidos BEGIN\n    UPDATE auditoria SET visto = 1 WHERE pedido = new.id;\nEND;\nCREATE VIEW v_tipos AS SELECT tipo, count(*) AS n FROM eventos GROUP BY tipo;\nCREATE VIEW v_auditoria AS SELECT * FROM auditoria;\nCREATE TRIGGER tg_v_auditoria INSTEAD OF INSERT ON v_auditoria BEGIN SELECT 1; END;\n",
        esperado: Esperado::Erro {
            mensagem: "A coluna visto é usada pelo gatilho tg_pedidos_visto (de pedidos). Exclua o gatilho antes.",
            status: 400,
        },
    },
    Caso {
        nome: "excluir_erro_eventos_tipo_visao",
        setup: "\nCREATE TABLE auditoria (id INTEGER PRIMARY KEY, pedido INTEGER, mensagem TEXT, visto INTEGER);\nCREATE TRIGGER tg_pedidos_auditoria AFTER INSERT ON pedidos BEGIN\n    INSERT INTO auditoria (pedido, mensagem) VALUES (new.id, 'criado');\nEND;\nCREATE TRIGGER tg_pedidos_visto AFTER UPDATE ON pedidos BEGIN\n    UPDATE auditoria SET visto = 1 WHERE pedido = new.id;\nEND;\nCREATE VIEW v_tipos AS SELECT tipo, count(*) AS n FROM eventos GROUP BY tipo;\nCREATE VIEW v_auditoria AS SELECT * FROM auditoria;\nCREATE TRIGGER tg_v_auditoria INSTEAD OF INSERT ON v_auditoria BEGIN SELECT 1; END;\n",
        esperado: Esperado::Erro {
            mensagem: "A coluna tipo é usada pela visão v_tipos. Exclua a visão antes.",
            status: 400,
        },
    },
    Caso {
        nome: "excluir_erro_unica_coluna",
        setup: "\nCREATE TABLE t_desc (id INTEGER PRIMARY KEY DESC, v TEXT);\nCREATE TABLE t_comp (a INTEGER, b TEXT, PRIMARY KEY (a));\nCREATE TABLE t_oid (rowid TEXT, oid TEXT, v TEXT);\nCREATE TABLE t_unica (so TEXT);\nINSERT INTO t_desc VALUES (5, 'x'); INSERT INTO t_comp VALUES (1, 'b'); INSERT INTO t_oid VALUES ('r', 'o', 'v');\n",
        esperado: Esperado::Erro {
            mensagem: "A coluna so é a única de t_unica; uma tabela precisa de ao menos uma coluna. Se for o caso, exclua a tabela.",
            status: 400,
        },
    },
    Caso {
        nome: "excluir_erro_referenciada",
        setup: "\nCREATE TABLE marcas (id INTEGER PRIMARY KEY, sigla TEXT UNIQUE COLLATE NOCASE);\nCREATE TABLE modelos (id INTEGER PRIMARY KEY, sigla TEXT REFERENCES marcas (sigla));\nINSERT INTO marcas VALUES (1, 'ABC');\nINSERT INTO modelos VALUES (1, 'abc');\n",
        esperado: Esperado::Erro {
            mensagem: "A coluna sigla não aceita repetição (UNIQUE). Tire essa regra da coluna antes (isso reconstrói a tabela).",
            status: 400,
        },
    },
    Caso {
        nome: "excluir_erro_fk",
        setup: "DROP INDEX idx_pedidos_cliente;",
        esperado: Esperado::Erro {
            mensagem: "A coluna cliente_id é uma chave estrangeira para clientes. Tire a referência antes.",
            status: 400,
        },
    },
    Caso {
        nome: "excluir_erro_restricao",
        setup: "CREATE TABLE faixas (de INTEGER, ate INTEGER, passo INTEGER CHECK (passo <= ate), nota TEXT, CHECK (de < ate));",
        esperado: Esperado::Erro {
            mensagem: "A coluna de aparece na restrição da tabela «CHECK (de < ate)». Tire a restrição antes.",
            status: 400,
        },
    },
    Caso {
        nome: "excluir_erro_check_outra",
        setup: "CREATE TABLE faixas (de INTEGER, ate INTEGER, passo INTEGER CHECK (passo <= ate), nota TEXT);",
        esperado: Esperado::Erro {
            mensagem: "A regra de verificação da coluna passo usa ate. Mude a regra antes.",
            status: 400,
        },
    },
    Caso {
        nome: "excluir_erro_unique_tabela",
        setup: "CREATE TABLE u (a TEXT, b TEXT, UNIQUE (a, b));",
        esperado: Esperado::Erro {
            mensagem: "A coluna a faz parte de uma restrição UNIQUE da tabela (sqlite_autoindex_u_1). Tire a restrição antes.",
            status: 400,
        },
    },
    Caso {
        nome: "excluir_erro_gatilho_le",
        setup: "CREATE TABLE g (a TEXT, b TEXT); CREATE TRIGGER tg_g AFTER INSERT ON g BEGIN SELECT new.b; END;",
        esperado: Esperado::Erro {
            mensagem: "A coluna b é usada pelo gatilho tg_g. Exclua o gatilho antes.",
            status: 400,
        },
    },
    Caso {
        nome: "excluir_erro_coluna_nao_existe",
        setup: "",
        esperado: Esperado::Erro { mensagem: "A coluna xyz não existe em eventos.", status: 404 },
    },
    Caso {
        nome: "reordenar_eventos",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Reordenar as colunas de eventos: dados, tipo, momento, valor. A tabela será reconstruída.",
            comandos: &[
                "BEGIN",
                "CREATE TABLE \"_novo_eventos\" (\n    \"dados\" TEXT,\n    \"tipo\" TEXT,\n    \"momento\" TEXT,\n    \"valor\" REAL\n)",
                "INSERT INTO \"_novo_eventos\" (rowid, \"dados\", \"tipo\", \"momento\", \"valor\")\n    SELECT rowid, \"dados\", \"tipo\", \"momento\", \"valor\" FROM \"eventos\"",
                "DROP TABLE \"eventos\"",
                "ALTER TABLE \"_novo_eventos\" RENAME TO \"eventos\"",
                "PRAGMA foreign_key_check(\"eventos\")",
                "COMMIT",
            ],
            reconstroi: true,
            avisos: &[
                "A tabela será reconstruída: os dados são copiados para uma tabela nova com a definição alterada, e os índices, gatilhos e visões ligados a ela são recriados.",
                "Durante a reconstrução as chaves estrangeiras ficam desligadas (PRAGMA foreign_keys = OFF) e o ALTER TABLE usa o modo antigo (PRAGMA legacy_alter_table = ON); os dois voltam como estavam no fim.",
            ],
        },
    },
    Caso {
        nome: "reordenar_produtos",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Reordenar as colunas de produtos: preco_centavos, serie, foto, estoque, preco, descricao, codigo. A tabela será reconstruída.",
            comandos: &[
                "BEGIN",
                "CREATE TABLE \"_novo_produtos\" (\n    \"preco_centavos\" INTEGER GENERATED ALWAYS AS (CAST(preco * 100 AS INTEGER)) VIRTUAL,\n    \"serie\" INTEGER,\n    \"foto\" BLOB,\n    \"estoque\" INTEGER DEFAULT 0,\n    \"preco\" NUMERIC NOT NULL DEFAULT 0,\n    \"descricao\" TEXT NOT NULL COLLATE NOCASE,\n    \"codigo\" TEXT PRIMARY KEY\n) WITHOUT ROWID",
                "INSERT INTO \"_novo_produtos\" (\"serie\", \"foto\", \"estoque\", \"preco\", \"descricao\", \"codigo\")\n    SELECT \"serie\", \"foto\", \"estoque\", \"preco\", \"descricao\", \"codigo\" FROM \"produtos\"",
                "DROP TABLE \"produtos\"",
                "ALTER TABLE \"_novo_produtos\" RENAME TO \"produtos\"",
                "PRAGMA foreign_key_check(\"produtos\")",
                "PRAGMA foreign_key_check(\"itens_pedido\")",
                "COMMIT",
            ],
            reconstroi: true,
            avisos: &[
                "A tabela será reconstruída: os dados são copiados para uma tabela nova com a definição alterada, e os índices, gatilhos e visões ligados a ela são recriados.",
                "Durante a reconstrução as chaves estrangeiras ficam desligadas (PRAGMA foreign_keys = OFF) e o ALTER TABLE usa o modo antigo (PRAGMA legacy_alter_table = ON); os dois voltam como estavam no fim.",
            ],
        },
    },
    Caso {
        nome: "reordenar_itens",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Reordenar as colunas de itens_pedido: produto, pedido_id, quantidade, preco. A tabela será reconstruída.",
            comandos: &[
                "BEGIN",
                "CREATE TABLE \"_novo_itens_pedido\" (\n    \"produto\" TEXT NOT NULL REFERENCES \"produtos\" (\"codigo\"),\n    \"pedido_id\" INTEGER NOT NULL REFERENCES \"pedidos\" (\"id\") ON DELETE CASCADE,\n    \"quantidade\" INTEGER NOT NULL DEFAULT 1,\n    \"preco\" NUMERIC,\n    PRIMARY KEY (pedido_id, produto)\n)",
                "INSERT INTO \"_novo_itens_pedido\" (rowid, \"produto\", \"pedido_id\", \"quantidade\", \"preco\")\n    SELECT rowid, \"produto\", \"pedido_id\", \"quantidade\", \"preco\" FROM \"itens_pedido\"",
                "DROP TABLE \"itens_pedido\"",
                "ALTER TABLE \"_novo_itens_pedido\" RENAME TO \"itens_pedido\"",
                "PRAGMA foreign_key_check(\"itens_pedido\")",
                "COMMIT",
            ],
            reconstroi: true,
            avisos: &[
                "A tabela será reconstruída: os dados são copiados para uma tabela nova com a definição alterada, e os índices, gatilhos e visões ligados a ela são recriados.",
                "Durante a reconstrução as chaves estrangeiras ficam desligadas (PRAGMA foreign_keys = OFF) e o ALTER TABLE usa o modo antigo (PRAGMA legacy_alter_table = ON); os dois voltam como estavam no fim.",
            ],
        },
    },
    Caso {
        nome: "reordenar_clientes",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Reordenar as colunas de clientes: criado_em, ativo, limite, cidade, email, nome, id. A tabela será reconstruída.",
            comandos: &[
                "BEGIN",
                "CREATE TABLE \"_novo_clientes\" (\n    \"criado_em\" TEXT DEFAULT CURRENT_TIMESTAMP,\n    \"ativo\" INTEGER NOT NULL DEFAULT 1,\n    \"limite\" REAL CHECK (limite >= 0),\n    \"cidade\" TEXT DEFAULT 'São Paulo',\n    \"email\" TEXT UNIQUE,\n    \"nome\" TEXT NOT NULL,\n    \"id\" INTEGER PRIMARY KEY AUTOINCREMENT\n)",
                "INSERT INTO \"_novo_clientes\" (\"criado_em\", \"ativo\", \"limite\", \"cidade\", \"email\", \"nome\", \"id\")\n    SELECT \"criado_em\", \"ativo\", \"limite\", \"cidade\", \"email\", \"nome\", \"id\" FROM \"clientes\"",
                "DELETE FROM sqlite_sequence WHERE name = '_novo_clientes'",
                "INSERT INTO sqlite_sequence (name, seq) SELECT '_novo_clientes', seq FROM sqlite_sequence WHERE name = 'clientes'",
                "DROP VIEW \"v_pedidos_por_cliente\"",
                "DROP TABLE \"clientes\"",
                "ALTER TABLE \"_novo_clientes\" RENAME TO \"clientes\"",
                "CREATE UNIQUE INDEX idx_clientes_nome_cidade ON clientes (nome, cidade) WHERE ativo = 1",
                "CREATE VIEW v_pedidos_por_cliente AS\n    SELECT c.id, c.nome, count(p.id) AS pedidos, max(p.feito_em) AS ultimo\n    FROM clientes c LEFT JOIN pedidos p ON p.cliente_id = c.id\n    GROUP BY c.id",
                "PRAGMA foreign_key_check(\"clientes\")",
                "PRAGMA foreign_key_check(\"pedidos\")",
                "COMMIT",
            ],
            reconstroi: true,
            avisos: &[
                "A tabela será reconstruída: os dados são copiados para uma tabela nova com a definição alterada, e os índices, gatilhos e visões ligados a ela são recriados.",
                "Durante a reconstrução as chaves estrangeiras ficam desligadas (PRAGMA foreign_keys = OFF) e o ALTER TABLE usa o modo antigo (PRAGMA legacy_alter_table = ON); os dois voltam como estavam no fim.",
            ],
        },
    },
    Caso {
        nome: "reordenar_pedidos",
        setup: "\nCREATE TABLE auditoria (id INTEGER PRIMARY KEY, pedido INTEGER, mensagem TEXT, visto INTEGER);\nCREATE TRIGGER tg_pedidos_auditoria AFTER INSERT ON pedidos BEGIN\n    INSERT INTO auditoria (pedido, mensagem) VALUES (new.id, 'criado');\nEND;\nCREATE TRIGGER tg_pedidos_visto AFTER UPDATE ON pedidos BEGIN\n    UPDATE auditoria SET visto = 1 WHERE pedido = new.id;\nEND;\nCREATE VIEW v_tipos AS SELECT tipo, count(*) AS n FROM eventos GROUP BY tipo;\nCREATE VIEW v_auditoria AS SELECT * FROM auditoria;\nCREATE TRIGGER tg_v_auditoria INSTEAD OF INSERT ON v_auditoria BEGIN SELECT 1; END;\n",
        esperado: Esperado::Plano {
            descricao: "Reordenar as colunas de pedidos: observacao, status, feito_em, cliente_id, id. A tabela será reconstruída.",
            comandos: &[
                "BEGIN",
                "CREATE TABLE \"_novo_pedidos\" (\n    \"observacao\" TEXT,\n    \"status\" TEXT NOT NULL CHECK (status IN ('aberto', 'pago', 'cancelado')) DEFAULT 'aberto',\n    \"feito_em\" TEXT NOT NULL,\n    \"cliente_id\" INTEGER NOT NULL REFERENCES \"clientes\" (\"id\") ON DELETE CASCADE,\n    \"id\" INTEGER PRIMARY KEY\n)",
                "INSERT INTO \"_novo_pedidos\" (\"observacao\", \"status\", \"feito_em\", \"cliente_id\", \"id\")\n    SELECT \"observacao\", \"status\", \"feito_em\", \"cliente_id\", \"id\" FROM \"pedidos\"",
                "DROP VIEW \"v_pedidos_por_cliente\"",
                "DROP TABLE \"pedidos\"",
                "ALTER TABLE \"_novo_pedidos\" RENAME TO \"pedidos\"",
                "CREATE INDEX idx_pedidos_cliente ON pedidos (cliente_id)",
                "CREATE INDEX idx_pedidos_status_data ON pedidos (status, feito_em DESC)",
                "CREATE TRIGGER trg_pedido_cancelado AFTER UPDATE OF status ON pedidos\n    WHEN new.status = 'cancelado'\nBEGIN\n    UPDATE pedidos SET observacao = coalesce(observacao || ' · ', '') || 'cancelado' WHERE id = new.id;\nEND",
                "CREATE TRIGGER tg_pedidos_auditoria AFTER INSERT ON pedidos BEGIN\n    INSERT INTO auditoria (pedido, mensagem) VALUES (new.id, 'criado');\nEND",
                "CREATE TRIGGER tg_pedidos_visto AFTER UPDATE ON pedidos BEGIN\n    UPDATE auditoria SET visto = 1 WHERE pedido = new.id;\nEND",
                "CREATE VIEW v_pedidos_por_cliente AS\n    SELECT c.id, c.nome, count(p.id) AS pedidos, max(p.feito_em) AS ultimo\n    FROM clientes c LEFT JOIN pedidos p ON p.cliente_id = c.id\n    GROUP BY c.id",
                "PRAGMA foreign_key_check(\"pedidos\")",
                "PRAGMA foreign_key_check(\"itens_pedido\")",
                "COMMIT",
            ],
            reconstroi: true,
            avisos: &[
                "A tabela será reconstruída: os dados são copiados para uma tabela nova com a definição alterada, e os índices, gatilhos e visões ligados a ela são recriados.",
                "Durante a reconstrução as chaves estrangeiras ficam desligadas (PRAGMA foreign_keys = OFF) e o ALTER TABLE usa o modo antigo (PRAGMA legacy_alter_table = ON); os dois voltam como estavam no fim.",
            ],
        },
    },
    Caso {
        nome: "reordenar_auditoria",
        setup: "\nCREATE TABLE auditoria (id INTEGER PRIMARY KEY, pedido INTEGER, mensagem TEXT, visto INTEGER);\nCREATE TRIGGER tg_pedidos_auditoria AFTER INSERT ON pedidos BEGIN\n    INSERT INTO auditoria (pedido, mensagem) VALUES (new.id, 'criado');\nEND;\nCREATE TRIGGER tg_pedidos_visto AFTER UPDATE ON pedidos BEGIN\n    UPDATE auditoria SET visto = 1 WHERE pedido = new.id;\nEND;\nCREATE VIEW v_tipos AS SELECT tipo, count(*) AS n FROM eventos GROUP BY tipo;\nCREATE VIEW v_auditoria AS SELECT * FROM auditoria;\nCREATE TRIGGER tg_v_auditoria INSTEAD OF INSERT ON v_auditoria BEGIN SELECT 1; END;\n",
        esperado: Esperado::Plano {
            descricao: "Reordenar as colunas de auditoria: visto, mensagem, pedido, id. A tabela será reconstruída.",
            comandos: &[
                "BEGIN",
                "CREATE TABLE \"_novo_auditoria\" (\n    \"visto\" INTEGER,\n    \"mensagem\" TEXT,\n    \"pedido\" INTEGER,\n    \"id\" INTEGER PRIMARY KEY\n)",
                "INSERT INTO \"_novo_auditoria\" (\"visto\", \"mensagem\", \"pedido\", \"id\")\n    SELECT \"visto\", \"mensagem\", \"pedido\", \"id\" FROM \"auditoria\"",
                "DROP VIEW \"v_auditoria\"",
                "DROP TABLE \"auditoria\"",
                "ALTER TABLE \"_novo_auditoria\" RENAME TO \"auditoria\"",
                "CREATE VIEW v_auditoria AS SELECT * FROM auditoria",
                "CREATE TRIGGER tg_v_auditoria INSTEAD OF INSERT ON v_auditoria BEGIN SELECT 1; END",
                "PRAGMA foreign_key_check(\"auditoria\")",
                "COMMIT",
            ],
            reconstroi: true,
            avisos: &[
                "A tabela será reconstruída: os dados são copiados para uma tabela nova com a definição alterada, e os índices, gatilhos e visões ligados a ela são recriados.",
                "Durante a reconstrução as chaves estrangeiras ficam desligadas (PRAGMA foreign_keys = OFF) e o ALTER TABLE usa o modo antigo (PRAGMA legacy_alter_table = ON); os dois voltam como estavam no fim.",
            ],
        },
    },
    Caso {
        nome: "reordenar_registro",
        setup: "\nCREATE TABLE categoria (id INTEGER PRIMARY KEY, nome TEXT);\nCREATE TABLE registro (\n  id INTEGER CONSTRAINT pk_registro PRIMARY KEY -- chave\n      AUTOINCREMENT,\n  categoria_id INTEGER CONSTRAINT fk_cat REFERENCES categoria (id) -- dono\n      ON DELETE CASCADE,\n  nota TEXT\n);\nINSERT INTO categoria VALUES (1, 'Raiz'), (2, 'Outra');\nINSERT INTO registro (categoria_id, nota) VALUES (1, 'a'), (1, 'b'), (2, 'c');\nDELETE FROM registro WHERE id = 3;\n",
        esperado: Esperado::Plano {
            descricao: "Reordenar as colunas de registro: nota, categoria_id, id. A tabela será reconstruída.",
            comandos: &[
                "BEGIN",
                "CREATE TABLE \"_novo_registro\" (\n    \"nota\" TEXT,\n    \"categoria_id\" INTEGER CONSTRAINT fk_cat REFERENCES categoria (id) /* dono */ ON DELETE CASCADE,\n    \"id\" INTEGER CONSTRAINT pk_registro PRIMARY KEY /* chave */ AUTOINCREMENT\n)",
                "INSERT INTO \"_novo_registro\" (\"nota\", \"categoria_id\", \"id\")\n    SELECT \"nota\", \"categoria_id\", \"id\" FROM \"registro\"",
                "DELETE FROM sqlite_sequence WHERE name = '_novo_registro'",
                "INSERT INTO sqlite_sequence (name, seq) SELECT '_novo_registro', seq FROM sqlite_sequence WHERE name = 'registro'",
                "DROP TABLE \"registro\"",
                "ALTER TABLE \"_novo_registro\" RENAME TO \"registro\"",
                "PRAGMA foreign_key_check(\"registro\")",
                "COMMIT",
            ],
            reconstroi: true,
            avisos: &[
                "A tabela será reconstruída: os dados são copiados para uma tabela nova com a definição alterada, e os índices, gatilhos e visões ligados a ela são recriados.",
                "Durante a reconstrução as chaves estrangeiras ficam desligadas (PRAGMA foreign_keys = OFF) e o ALTER TABLE usa o modo antigo (PRAGMA legacy_alter_table = ON); os dois voltam como estavam no fim.",
            ],
        },
    },
    Caso {
        nome: "reordenar_espaco",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Reordenar as colunas de Tabela com espaço: número, coluna \"citada\". A tabela será reconstruída.",
            comandos: &[
                "BEGIN",
                "CREATE TABLE \"_novo_Tabela com espaço\" (\n    \"número\" INTEGER,\n    \"coluna \"\"citada\"\"\" TEXT\n)",
                "INSERT INTO \"_novo_Tabela com espaço\" (rowid, \"número\", \"coluna \"\"citada\"\"\")\n    SELECT rowid, \"número\", \"coluna \"\"citada\"\"\" FROM \"Tabela com espaço\"",
                "DROP TABLE \"Tabela com espaço\"",
                "ALTER TABLE \"_novo_Tabela com espaço\" RENAME TO \"Tabela com espaço\"",
                "PRAGMA foreign_key_check(\"Tabela com espaço\")",
                "COMMIT",
            ],
            reconstroi: true,
            avisos: &[
                "A tabela será reconstruída: os dados são copiados para uma tabela nova com a definição alterada, e os índices, gatilhos e visões ligados a ela são recriados.",
                "Durante a reconstrução as chaves estrangeiras ficam desligadas (PRAGMA foreign_keys = OFF) e o ALTER TABLE usa o modo antigo (PRAGMA legacy_alter_table = ON); os dois voltam como estavam no fim.",
            ],
        },
    },
    Caso {
        nome: "reordenar_mesma_ordem",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "A ordem das colunas de eventos não muda.",
            comandos: &[],
            reconstroi: false,
            avisos: &[],
        },
    },
    Caso {
        nome: "reordenar_erro_faltando",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "A nova ordem precisa ter exatamente as mesmas colunas da tabela, cada uma uma vez.",
            status: 400,
        },
    },
    Caso {
        nome: "reordenar_erro_repetida",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "A nova ordem precisa ter exatamente as mesmas colunas da tabela, cada uma uma vez.",
            status: 400,
        },
    },
    Caso {
        nome: "criar_tarefas",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Criar a tabela tarefas com 3 colunas.",
            comandos: &[
                "CREATE TABLE \"tarefas\" (\n    \"id\" INTEGER PRIMARY KEY AUTOINCREMENT,\n    \"titulo\" TEXT NOT NULL DEFAULT 'sem título',\n    \"cliente_id\" INTEGER REFERENCES \"clientes\" (\"id\") ON DELETE SET NULL\n)",
            ],
            reconstroi: false,
            avisos: &[],
        },
    },
    Caso {
        nome: "criar_pares",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Criar a tabela pares com 2 colunas.",
            comandos: &[
                "CREATE TABLE \"pares\" (\n    \"a\" TEXT,\n    \"b\" INTEGER,\n    PRIMARY KEY (\"a\", \"b\")\n) WITHOUT ROWID",
            ],
            reconstroi: false,
            avisos: &["As colunas a, b formam juntas a chave primária."],
        },
    },
    Caso {
        nome: "criar_faixas",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Criar a tabela faixas com 4 colunas.",
            comandos: &[
                "CREATE TABLE \"faixas\" (\n    \"de\" INTEGER,\n    \"ate\" INTEGER,\n    \"passo\" INTEGER CHECK (passo <= ate),\n    \"nota\" TEXT COLLATE NOCASE,\n    CHECK (de < ate),\n    UNIQUE (de, ate)\n)",
            ],
            reconstroi: false,
            avisos: &[],
        },
    },
    Caso {
        nome: "criar_uma",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Criar a tabela uma com 1 coluna.",
            comandos: &["CREATE TABLE \"uma\" (\n    \"a\"\n)"],
            reconstroi: false,
            avisos: &[],
        },
    },
    Caso {
        nome: "criar_sem_rowid_com_restricao",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Criar a tabela sr com 1 coluna.",
            comandos: &["CREATE TABLE \"sr\" (\n    \"a\" TEXT,\n    PRIMARY KEY (a)\n) WITHOUT ROWID"],
            reconstroi: false,
            avisos: &[],
        },
    },
    Caso {
        nome: "criar_nomes_citados",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Criar a tabela nova \"tabela\" com 2 colunas.",
            comandos: &[
                "CREATE TABLE \"nova \"\"tabela\"\"\" (\n    \"id \"\"x\"\"\" INTEGER PRIMARY KEY,\n    \"valor; texto\" TEXT\n)",
            ],
            reconstroi: false,
            avisos: &[],
        },
    },
    Caso {
        nome: "criar_erro_sem_nome",
        setup: "",
        esperado: Esperado::Erro { mensagem: "Informe o nome da tabela.", status: 400 },
    },
    Caso {
        nome: "criar_erro_existe",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "Já existe uma tabela chamado clientes no banco (o SQLite não diferencia maiúsculas de minúsculas nos nomes).",
            status: 409,
        },
    },
    Caso {
        nome: "criar_erro_existe_indice",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "Já existe um índice chamado idx_pedidos_cliente no banco (o SQLite não diferencia maiúsculas de minúsculas nos nomes).",
            status: 409,
        },
    },
    Caso {
        nome: "criar_erro_existe_visao",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "Já existe uma visão chamado v_pedidos_por_cliente no banco (o SQLite não diferencia maiúsculas de minúsculas nos nomes).",
            status: 409,
        },
    },
    Caso {
        nome: "criar_erro_reservado",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "Nomes que começam com sqlite_ são reservados pelo SQLite.", status: 400
        },
    },
    Caso {
        nome: "criar_erro_sem_colunas",
        setup: "",
        esperado: Esperado::Erro { mensagem: "A tabela precisa de ao menos uma coluna.", status: 400 },
    },
    Caso {
        nome: "criar_erro_repetida",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "A coluna A aparece mais de uma vez (o SQLite não diferencia maiúsculas de minúsculas nos nomes).",
            status: 400,
        },
    },
    Caso {
        nome: "criar_erro_coluna_sem_nome",
        setup: "",
        esperado: Esperado::Erro { mensagem: "Toda coluna precisa de um nome.", status: 400 },
    },
    Caso {
        nome: "criar_erro_auto_texto",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "Autoincremento só vale numa chave primária INTEGER de uma coluna só (coluna a).",
            status: 400,
        },
    },
    Caso {
        nome: "criar_erro_auto_sem_pk",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "Autoincremento só vale numa chave primária INTEGER de uma coluna só (coluna a).",
            status: 400,
        },
    },
    Caso {
        nome: "criar_erro_auto_sem_rowid",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "Tabela sem rowid (WITHOUT ROWID) não aceita autoincremento.",
            status: 400,
        },
    },
    Caso {
        nome: "criar_erro_sem_rowid_sem_chave",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "Uma tabela sem rowid (WITHOUT ROWID) precisa de chave primária.",
            status: 400,
        },
    },
    Caso {
        nome: "criar_erro_ref_invalida",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "A definição da tabela não é válida. Erro no comando SQL: near \")\": syntax error.",
            status: 400,
        },
    },
    Caso {
        nome: "renomear_clientes",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Renomear a tabela clientes para contatos.",
            comandos: &["ALTER TABLE \"clientes\" RENAME TO \"contatos\""],
            reconstroi: false,
            avisos: &["Chaves estrangeiras, índices, gatilhos e visões que citam a tabela passam a usar o nome novo."],
        },
    },
    Caso {
        nome: "renomear_eventos_maiusculas",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Renomear a tabela eventos para Eventos.",
            comandos: &[
                "BEGIN",
                "ALTER TABLE \"eventos\" RENAME TO \"_novo_eventos\"",
                "ALTER TABLE \"_novo_eventos\" RENAME TO \"Eventos\"",
                "COMMIT",
            ],
            reconstroi: false,
            avisos: &["Chaves estrangeiras, índices, gatilhos e visões que citam a tabela passam a usar o nome novo."],
        },
    },
    Caso {
        nome: "renomear_notas_virtual",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Renomear a tabela notas para anotacoes.",
            comandos: &["ALTER TABLE \"notas\" RENAME TO \"anotacoes\""],
            reconstroi: false,
            avisos: &["Chaves estrangeiras, índices, gatilhos e visões que citam a tabela passam a usar o nome novo."],
        },
    },
    Caso {
        nome: "renomear_erro_existe",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "Já existe uma tabela chamado pedidos no banco (o SQLite não diferencia maiúsculas de minúsculas nos nomes).",
            status: 409,
        },
    },
    Caso {
        nome: "renomear_erro_igual",
        setup: "",
        esperado: Esperado::Erro { mensagem: "O nome novo é igual ao atual.", status: 400 },
    },
    Caso {
        nome: "renomear_erro_vazio",
        setup: "",
        esperado: Esperado::Erro { mensagem: "Informe o nome novo da tabela.", status: 400 },
    },
    Caso {
        nome: "renomear_erro_visao",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "v_pedidos_por_cliente é uma visão: a estrutura dela é o próprio SELECT. Para mudá-la, exclua a visão e crie de novo.",
            status: 400,
        },
    },
    Caso {
        nome: "renomear_erro_reservado",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "Nomes que começam com sqlite_ são reservados pelo SQLite.", status: 400
        },
    },
    Caso {
        nome: "excluir_tabela_eventos",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Excluir a tabela eventos e as 5.000 linhas dela.",
            comandos: &["DROP TABLE \"eventos\""],
            reconstroi: false,
            avisos: &["A exclusão não pode ser desfeita."],
        },
    },
    Caso {
        nome: "excluir_tabela_pedidos",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Excluir a tabela pedidos e as 240 linhas dela.",
            comandos: &["DROP TABLE \"pedidos\""],
            reconstroi: false,
            avisos: &[
                "A exclusão não pode ser desfeita.",
                "Saem junto 2 índices e 1 gatilho.",
                "A tabela itens_pedido tem chave estrangeira para pedidos com ON DELETE CASCADE: 456 linhas ligadas de itens_pedido também serão excluídas.",
                "Depois da exclusão, a chave estrangeira de itens_pedido fica apontando para uma tabela que não existe: com as chaves estrangeiras ligadas, gravar em itens_pedido passa a falhar até a referência ser tirada.",
                "Estas visões citam a tabela e deixarão de funcionar: v_pedidos_por_cliente.",
            ],
        },
    },
    Caso {
        nome: "excluir_tabela_clientes",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Excluir a tabela clientes e as 60 linhas dela.",
            comandos: &["DROP TABLE \"clientes\""],
            reconstroi: false,
            avisos: &[
                "A exclusão não pode ser desfeita.",
                "Saem junto 1 índice e 0 gatilhos.",
                "A tabela pedidos tem chave estrangeira para clientes com ON DELETE CASCADE: 240 linhas ligadas de pedidos também serão excluídas.",
                "Depois da exclusão, a chave estrangeira de pedidos fica apontando para uma tabela que não existe: com as chaves estrangeiras ligadas, gravar em pedidos passa a falhar até a referência ser tirada.",
                "Estas visões citam a tabela e deixarão de funcionar: v_pedidos_por_cliente.",
            ],
        },
    },
    Caso {
        nome: "excluir_tabela_produtos",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Excluir a tabela produtos e as 10 linhas dela.",
            comandos: &["DROP TABLE \"produtos\""],
            reconstroi: false,
            avisos: &[
                "A exclusão não pode ser desfeita.",
                "A tabela itens_pedido tem chave estrangeira para produtos (NO ACTION): a exclusão falha enquanto houver linhas de itens_pedido apontando para ela (hoje, 456 linhas).",
                "Depois da exclusão, a chave estrangeira de itens_pedido fica apontando para uma tabela que não existe: com as chaves estrangeiras ligadas, gravar em itens_pedido passa a falhar até a referência ser tirada.",
            ],
        },
    },
    Caso {
        nome: "excluir_tabela_notas",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Excluir a tabela notas e as 2 linhas dela.",
            comandos: &["DROP TABLE \"notas\""],
            reconstroi: false,
            avisos: &["A exclusão não pode ser desfeita."],
        },
    },
    Caso {
        nome: "excluir_tabela_autores",
        setup: "\nCREATE TABLE autores (id INTEGER PRIMARY KEY, nome TEXT);\nCREATE TABLE textos (id INTEGER PRIMARY KEY, titulo TEXT, autor_id INTEGER REFERENCES autores (id) ON DELETE SET NULL);\nCREATE TABLE resenhas (id INTEGER PRIMARY KEY, autor_id INTEGER REFERENCES autores (id));\nCREATE TABLE notas_autor (autor_id INTEGER REFERENCES autores (id) ON DELETE SET DEFAULT, texto TEXT);\nCREATE TABLE pares (a INTEGER, b TEXT, PRIMARY KEY (a, b));\nCREATE TABLE ligadas (x INTEGER, y TEXT, FOREIGN KEY (x, y) REFERENCES pares (a, b) ON DELETE CASCADE);\nINSERT INTO autores VALUES (1, 'Ana Exemplo'), (2, 'Bruno Exemplo');\nINSERT INTO textos VALUES (1, 'A', 1), (2, 'B', 2), (3, 'C', 1), (4, 'D', NULL);\nINSERT INTO notas_autor VALUES (1, 'x');\nINSERT INTO pares VALUES (1, 'a'), (2, 'b');\nINSERT INTO ligadas VALUES (1, 'a');\n",
        esperado: Esperado::Plano {
            descricao: "Excluir a tabela autores e as 2 linhas dela.",
            comandos: &["DROP TABLE \"autores\""],
            reconstroi: false,
            avisos: &[
                "A exclusão não pode ser desfeita.",
                "A tabela textos tem chave estrangeira para autores com ON DELETE SET NULL: autor_id fica nulo em 3 linhas ligadas de textos.",
                "A tabela notas_autor tem chave estrangeira para autores com ON DELETE SET DEFAULT: autor_id volta ao valor padrão em 1 linha ligada de notas_autor (se o padrão não for nulo, a exclusão falha, porque ele deixa de ter para onde apontar).",
                "Depois da exclusão, a chave estrangeira de textos fica apontando para uma tabela que não existe: com as chaves estrangeiras ligadas, gravar em textos passa a falhar até a referência ser tirada.",
                "Depois da exclusão, a chave estrangeira de resenhas fica apontando para uma tabela que não existe: com as chaves estrangeiras ligadas, gravar em resenhas passa a falhar até a referência ser tirada.",
                "Depois da exclusão, a chave estrangeira de notas_autor fica apontando para uma tabela que não existe: com as chaves estrangeiras ligadas, gravar em notas_autor passa a falhar até a referência ser tirada.",
            ],
        },
    },
    Caso {
        nome: "excluir_tabela_pares",
        setup: "\nCREATE TABLE autores (id INTEGER PRIMARY KEY, nome TEXT);\nCREATE TABLE textos (id INTEGER PRIMARY KEY, titulo TEXT, autor_id INTEGER REFERENCES autores (id) ON DELETE SET NULL);\nCREATE TABLE resenhas (id INTEGER PRIMARY KEY, autor_id INTEGER REFERENCES autores (id));\nCREATE TABLE notas_autor (autor_id INTEGER REFERENCES autores (id) ON DELETE SET DEFAULT, texto TEXT);\nCREATE TABLE pares (a INTEGER, b TEXT, PRIMARY KEY (a, b));\nCREATE TABLE ligadas (x INTEGER, y TEXT, FOREIGN KEY (x, y) REFERENCES pares (a, b) ON DELETE CASCADE);\nINSERT INTO autores VALUES (1, 'Ana Exemplo'), (2, 'Bruno Exemplo');\nINSERT INTO textos VALUES (1, 'A', 1), (2, 'B', 2), (3, 'C', 1), (4, 'D', NULL);\nINSERT INTO notas_autor VALUES (1, 'x');\nINSERT INTO pares VALUES (1, 'a'), (2, 'b');\nINSERT INTO ligadas VALUES (1, 'a');\n",
        esperado: Esperado::Plano {
            descricao: "Excluir a tabela pares e as 2 linhas dela.",
            comandos: &["DROP TABLE \"pares\""],
            reconstroi: false,
            avisos: &[
                "A exclusão não pode ser desfeita.",
                "A tabela ligadas tem chave estrangeira para pares com ON DELETE CASCADE: 1 linha ligada de ligadas também será excluída.",
                "Depois da exclusão, a chave estrangeira de ligadas fica apontando para uma tabela que não existe: com as chaves estrangeiras ligadas, gravar em ligadas passa a falhar até a referência ser tirada.",
            ],
        },
    },
    Caso {
        nome: "excluir_tabela_autores_sem_fk",
        setup: "PRAGMA foreign_keys = OFF;\nCREATE TABLE autores (id INTEGER PRIMARY KEY, nome TEXT);\nCREATE TABLE textos (id INTEGER PRIMARY KEY, titulo TEXT, autor_id INTEGER REFERENCES autores (id) ON DELETE SET NULL);\nCREATE TABLE resenhas (id INTEGER PRIMARY KEY, autor_id INTEGER REFERENCES autores (id));\nCREATE TABLE notas_autor (autor_id INTEGER REFERENCES autores (id) ON DELETE SET DEFAULT, texto TEXT);\nCREATE TABLE pares (a INTEGER, b TEXT, PRIMARY KEY (a, b));\nCREATE TABLE ligadas (x INTEGER, y TEXT, FOREIGN KEY (x, y) REFERENCES pares (a, b) ON DELETE CASCADE);\nINSERT INTO autores VALUES (1, 'Ana Exemplo'), (2, 'Bruno Exemplo');\nINSERT INTO textos VALUES (1, 'A', 1), (2, 'B', 2), (3, 'C', 1), (4, 'D', NULL);\nINSERT INTO notas_autor VALUES (1, 'x');\nINSERT INTO pares VALUES (1, 'a'), (2, 'b');\nINSERT INTO ligadas VALUES (1, 'a');\n",
        esperado: Esperado::Plano {
            descricao: "Excluir a tabela autores e as 2 linhas dela.",
            comandos: &["DROP TABLE \"autores\""],
            reconstroi: false,
            avisos: &[
                "A exclusão não pode ser desfeita.",
                "A tabela textos tem chave estrangeira para autores, mas a verificação de chaves estrangeiras está desligada: 3 linhas ligadas de textos ficam apontando para linhas que não existem.",
                "A tabela notas_autor tem chave estrangeira para autores, mas a verificação de chaves estrangeiras está desligada: 1 linha ligada de notas_autor fica apontando para linhas que não existem.",
            ],
        },
    },
    Caso {
        nome: "excluir_tabela_auditoria",
        setup: "\nCREATE TABLE auditoria (id INTEGER PRIMARY KEY, pedido INTEGER, mensagem TEXT, visto INTEGER);\nCREATE TRIGGER tg_pedidos_auditoria AFTER INSERT ON pedidos BEGIN\n    INSERT INTO auditoria (pedido, mensagem) VALUES (new.id, 'criado');\nEND;\nCREATE TRIGGER tg_pedidos_visto AFTER UPDATE ON pedidos BEGIN\n    UPDATE auditoria SET visto = 1 WHERE pedido = new.id;\nEND;\nCREATE VIEW v_tipos AS SELECT tipo, count(*) AS n FROM eventos GROUP BY tipo;\nCREATE VIEW v_auditoria AS SELECT * FROM auditoria;\nCREATE TRIGGER tg_v_auditoria INSTEAD OF INSERT ON v_auditoria BEGIN SELECT 1; END;\n",
        esperado: Esperado::Plano {
            descricao: "Excluir a tabela auditoria e as 0 linhas dela.",
            comandos: &["DROP TABLE \"auditoria\""],
            reconstroi: false,
            avisos: &[
                "A exclusão não pode ser desfeita.",
                "Estas visões citam a tabela e deixarão de funcionar: v_auditoria.",
                "Estes gatilhos de outras tabelas citam auditoria e passarão a falhar (e a impedir a gravação que os dispara): tg_pedidos_auditoria (de pedidos), tg_pedidos_visto (de pedidos).",
            ],
        },
    },
    Caso {
        nome: "excluir_tabela_erro_nao_existe",
        setup: "",
        esperado: Esperado::Erro { mensagem: "A tabela xyz não existe.", status: 404 },
    },
    Caso {
        nome: "excluir_tabela_erro_interna",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "sqlite_sequence é uma tabela interna do SQLite; a estrutura dela não pode ser mudada.",
            status: 403,
        },
    },
    Caso {
        nome: "excluir_tabela_erro_shadow",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "notas_data é uma tabela auxiliar de uma tabela virtual; mudar a estrutura dela estragaria a tabela virtual.",
            status: 403,
        },
    },
    Caso {
        nome: "indice_unico",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Criar o índice único idx_produtos_descricao em produtos (descricao).",
            comandos: &["CREATE UNIQUE INDEX \"idx_produtos_descricao\" ON \"produtos\" (\"descricao\")"],
            reconstroi: false,
            avisos: &[
                "O índice acelera buscas e ordenações por essas colunas; o arquivo fica um pouco maior.",
                "Com o índice único, o banco passa a recusar valores repetidos nessas colunas.",
            ],
        },
    },
    Caso {
        nome: "indice_desc",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Criar o índice idx_eventos_momento em eventos (tipo, momento DESC).",
            comandos: &["CREATE INDEX \"idx_eventos_momento\" ON \"eventos\" (\"tipo\", \"momento\" DESC)"],
            reconstroi: false,
            avisos: &["O índice acelera buscas e ordenações por essas colunas; o arquivo fica um pouco maior."],
        },
    },
    Caso {
        nome: "indice_parcial",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Criar o índice idx_pedidos_pagos em pedidos (feito_em), só nas linhas em que status = 'pago'.",
            comandos: &["CREATE INDEX \"idx_pedidos_pagos\" ON \"pedidos\" (\"feito_em\") WHERE status = 'pago'"],
            reconstroi: false,
            avisos: &["O índice acelera buscas e ordenações por essas colunas; o arquivo fica um pouco maior."],
        },
    },
    Caso {
        nome: "indice_espaco",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Criar o índice idx \"espaço\" em Tabela com espaço (coluna \"citada\" DESC).",
            comandos: &[
                "CREATE INDEX \"idx \"\"espaço\"\"\" ON \"Tabela com espaço\" (\"coluna \"\"citada\"\"\" DESC)",
            ],
            reconstroi: false,
            avisos: &["O índice acelera buscas e ordenações por essas colunas; o arquivo fica um pouco maior."],
        },
    },
    Caso {
        nome: "indice_erro_repetidos",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "Não é possível criar o índice único: há linhas com valores repetidos em tipo, momento DESC.",
            status: 400,
        },
    },
    Caso {
        nome: "indice_erro_repetido_um",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "Não é possível criar o índice único: há linhas com valores repetidos em tipo.",
            status: 400,
        },
    },
    Caso {
        nome: "indice_erro_coluna",
        setup: "",
        esperado: Esperado::Erro { mensagem: "A coluna inexistente não existe em eventos.", status: 400 },
    },
    Caso {
        nome: "indice_erro_coluna_vazia",
        setup: "",
        esperado: Esperado::Erro { mensagem: "A coluna  não existe em eventos.", status: 400 },
    },
    Caso {
        nome: "indice_erro_duas_vezes",
        setup: "",
        esperado: Esperado::Erro { mensagem: "A coluna tipo aparece mais de uma vez no índice.", status: 400 },
    },
    Caso {
        nome: "indice_erro_nome",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "Já existe uma tabela chamado clientes no banco (o SQLite não diferencia maiúsculas de minúsculas nos nomes).",
            status: 409,
        },
    },
    Caso {
        nome: "indice_erro_sem_colunas",
        setup: "",
        esperado: Esperado::Erro { mensagem: "Escolha ao menos uma coluna para o índice.", status: 400 },
    },
    Caso {
        nome: "indice_erro_tabela",
        setup: "",
        esperado: Esperado::Erro { mensagem: "A tabela nao_existe não existe.", status: 404 },
    },
    Caso {
        nome: "indice_erro_onde",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "A condição do índice não pode ter ponto e vírgula: tipo = 'a'; DROP TABLE eventos",
            status: 400,
        },
    },
    Caso {
        nome: "indice_erro_onde_coluna",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "Não foi possível criar o índice. A coluna não existe: xyz.",
            status: 400,
        },
    },
    Caso {
        nome: "excluir_indice",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Excluir o índice idx_pedidos_cliente de pedidos.",
            comandos: &["DROP INDEX \"idx_pedidos_cliente\""],
            reconstroi: false,
            avisos: &["Os dados não mudam; só as buscas que usavam o índice podem ficar mais lentas."],
        },
    },
    Caso {
        nome: "excluir_indice_erro_auto",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "Esse índice vem de uma restrição UNIQUE ou da chave primária; ele sai junto quando a restrição sai.",
            status: 400,
        },
    },
    Caso {
        nome: "excluir_indice_erro_nao_existe",
        setup: "",
        esperado: Esperado::Erro { mensagem: "O índice xyz não existe.", status: 404 },
    },
    Caso {
        nome: "excluir_indice_erro_vazio",
        setup: "",
        esperado: Esperado::Erro { mensagem: "Informe o nome do índice.", status: 400 },
    },
    Caso {
        nome: "visao_criar",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Criar a visão v_eventos_por_tipo.",
            comandos: &["CREATE VIEW \"v_eventos_por_tipo\" AS\nSELECT tipo, count(*) AS n FROM eventos GROUP BY tipo"],
            reconstroi: false,
            avisos: &["A visão guarda só a consulta; os dados continuam nas tabelas."],
        },
    },
    Caso {
        nome: "visao_criar_with",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Criar a visão v_w.",
            comandos: &["CREATE VIEW \"v_w\" AS\nWITH x AS (SELECT 1 AS a) SELECT a FROM x"],
            reconstroi: false,
            avisos: &["A visão guarda só a consulta; os dados continuam nas tabelas."],
        },
    },
    Caso {
        nome: "visao_erro_delete",
        setup: "",
        esperado: Esperado::Erro { mensagem: "A visão precisa de um único comando SELECT.", status: 400 },
    },
    Caso {
        nome: "visao_erro_dois",
        setup: "",
        esperado: Esperado::Erro { mensagem: "A visão precisa de um único comando SELECT.", status: 400 },
    },
    Caso {
        nome: "visao_erro_tabela",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "O SELECT da visão não é válido. A tabela não existe: tabela_que_nao_existe.",
            status: 400,
        },
    },
    Caso {
        nome: "visao_erro_coluna",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "O SELECT da visão não é válido. A coluna não existe: xyz.", status: 400
        },
    },
    Caso {
        nome: "visao_erro_sintaxe",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "O SELECT da visão não é válido. Erro no comando SQL: near \"FROM\": syntax error.",
            status: 400,
        },
    },
    Caso {
        nome: "visao_erro_vazio",
        setup: "",
        esperado: Esperado::Erro { mensagem: "Escreva o SELECT da visão.", status: 400 },
    },
    Caso {
        nome: "visao_erro_nome",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "Já existe uma visão chamado v_pedidos_por_cliente no banco (o SQLite não diferencia maiúsculas de minúsculas nos nomes).",
            status: 409,
        },
    },
    Caso {
        nome: "visao_excluir",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Excluir a visão v_pedidos_por_cliente.",
            comandos: &["DROP VIEW \"v_pedidos_por_cliente\""],
            reconstroi: false,
            avisos: &["Os dados das tabelas não mudam; só a consulta salva some."],
        },
    },
    Caso {
        nome: "visao_excluir_com_gatilho",
        setup: "\nCREATE TABLE auditoria (id INTEGER PRIMARY KEY, pedido INTEGER, mensagem TEXT, visto INTEGER);\nCREATE TRIGGER tg_pedidos_auditoria AFTER INSERT ON pedidos BEGIN\n    INSERT INTO auditoria (pedido, mensagem) VALUES (new.id, 'criado');\nEND;\nCREATE TRIGGER tg_pedidos_visto AFTER UPDATE ON pedidos BEGIN\n    UPDATE auditoria SET visto = 1 WHERE pedido = new.id;\nEND;\nCREATE VIEW v_tipos AS SELECT tipo, count(*) AS n FROM eventos GROUP BY tipo;\nCREATE VIEW v_auditoria AS SELECT * FROM auditoria;\nCREATE TRIGGER tg_v_auditoria INSTEAD OF INSERT ON v_auditoria BEGIN SELECT 1; END;\n",
        esperado: Esperado::Plano {
            descricao: "Excluir a visão v_auditoria.",
            comandos: &["DROP VIEW \"v_auditoria\""],
            reconstroi: false,
            avisos: &[
                "Os dados das tabelas não mudam; só a consulta salva some.",
                "Saem junto os gatilhos da visão: tg_v_auditoria.",
            ],
        },
    },
    Caso {
        nome: "visao_excluir_erro",
        setup: "",
        esperado: Esperado::Erro { mensagem: "A visão xyz não existe.", status: 404 },
    },
    Caso {
        nome: "visao_excluir_erro_vazio",
        setup: "",
        esperado: Esperado::Erro { mensagem: "Informe o nome da visão.", status: 400 },
    },
    Caso {
        nome: "gatilho_criar",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Criar o gatilho trg_evento_erro.",
            comandos: &[
                "CREATE TRIGGER trg_evento_erro AFTER INSERT ON eventos WHEN new.tipo = 'erro'\nBEGIN\n    UPDATE eventos SET dados = 'marcado; revisar' WHERE rowid = new.rowid;\nEND",
            ],
            reconstroi: false,
            avisos: &["O gatilho roda sozinho a cada alteração que combina com ele."],
        },
    },
    Caso {
        nome: "gatilho_criar_temp",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "Não foi possível criar o gatilho. O SQLite recusou a operação: temporary trigger may not have qualified name.",
            status: 400,
        },
    },
    Caso {
        nome: "gatilho_erro_vazio",
        setup: "",
        esperado: Esperado::Erro { mensagem: "Escreva o comando CREATE TRIGGER.", status: 400 },
    },
    Caso {
        nome: "gatilho_erro_outro",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "O gatilho precisa de um único comando CREATE TRIGGER … BEGIN … END.",
            status: 400,
        },
    },
    Caso {
        nome: "gatilho_erro_incompleto",
        setup: "",
        esperado: Esperado::Erro { mensagem: "O comando CREATE TRIGGER está incompleto.", status: 400 },
    },
    Caso {
        nome: "gatilho_erro_existe",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "Já existe um gatilho chamado trg_pedido_cancelado no banco (o SQLite não diferencia maiúsculas de minúsculas nos nomes).",
            status: 409,
        },
    },
    Caso {
        nome: "gatilho_erro_tabela",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "Não foi possível criar o gatilho. A tabela não existe: main.nao_existe.",
            status: 400,
        },
    },
    Caso {
        nome: "gatilho_erro_reservado",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "Nomes que começam com sqlite_ são reservados pelo SQLite.", status: 400
        },
    },
    Caso {
        nome: "gatilho_excluir",
        setup: "",
        esperado: Esperado::Plano {
            descricao: "Excluir o gatilho trg_pedido_cancelado de pedidos.",
            comandos: &["DROP TRIGGER \"trg_pedido_cancelado\""],
            reconstroi: false,
            avisos: &["Os dados não mudam; o gatilho só deixa de rodar nas próximas alterações."],
        },
    },
    Caso {
        nome: "gatilho_excluir_erro",
        setup: "",
        esperado: Esperado::Erro { mensagem: "O gatilho xyz não existe.", status: 404 },
    },
    Caso {
        nome: "gatilho_excluir_erro_vazio",
        setup: "",
        esperado: Esperado::Erro { mensagem: "Informe o nome do gatilho.", status: 400 },
    },
    Caso {
        nome: "definicoes_visao",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "v_pedidos_por_cliente é uma visão: a estrutura dela é o próprio SELECT. Para mudá-la, exclua a visão e crie de novo.",
            status: 400,
        },
    },
    Caso {
        nome: "definicoes_virtual",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "notas é uma tabela virtual: as colunas vêm do módulo que a criou e não podem ser editadas.",
            status: 400,
        },
    },
    Caso {
        nome: "definicoes_shadow",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "notas_data é uma tabela auxiliar de uma tabela virtual; mudar a estrutura dela estragaria a tabela virtual.",
            status: 403,
        },
    },
    Caso {
        nome: "definicoes_interna",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "sqlite_sequence é uma tabela interna do SQLite; a estrutura dela não pode ser mudada.",
            status: 403,
        },
    },
    Caso {
        nome: "definicoes_nao_existe",
        setup: "",
        esperado: Esperado::Erro { mensagem: "A tabela nao_existe não existe.", status: 404 },
    },
    Caso {
        nome: "visao_erro_nul",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "O SELECT da visão não é válido. O SQLite recusou a operação: the query contains a null character.",
            status: 400,
        },
    },
    Caso {
        nome: "indice_erro_onde_nul",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "Não foi possível criar o índice. O SQLite recusou a operação: the query contains a null character.",
            status: 400,
        },
    },
    Caso {
        nome: "adicionar_erro_extra_nul",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "Não foi possível adicionar a coluna. O SQLite recusou a operação: the query contains a null character.",
            status: 400,
        },
    },
    Caso {
        nome: "adicionar_erro_complemento_nul",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "Não foi possível adicionar a coluna. O SQLite recusou a operação: the query contains a null character.",
            status: 400,
        },
    },
    Caso {
        nome: "adicionar_erro_padrao_nul",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "Não foi possível adicionar a coluna. O SQLite recusou a operação: the query contains a null character.",
            status: 400,
        },
    },
    Caso {
        nome: "adicionar_erro_nome_nul",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "Não foi possível adicionar a coluna. O SQLite recusou a operação: the query contains a null character.",
            status: 400,
        },
    },
    Caso {
        nome: "adicionar_erro_tipo_nul",
        setup: "",
        esperado: Esperado::Erro { mensagem: "O tipo «TEXT\0» não é válido.", status: 400 },
    },
    Caso {
        nome: "adicionar_erro_colacao_nul",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "Não foi possível adicionar a coluna. O SQLite recusou a operação: the query contains a null character.",
            status: 400,
        },
    },
    Caso {
        nome: "adicionar_erro_gerada_nul",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "Não foi possível adicionar a coluna. O SQLite recusou a operação: the query contains a null character.",
            status: 400,
        },
    },
    Caso {
        nome: "adicionar_erro_ref_tabela_nul",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "Não foi possível adicionar a coluna. O SQLite recusou a operação: the query contains a null character.",
            status: 400,
        },
    },
    Caso {
        nome: "criar_erro_check_nul",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "A definição da tabela não é válida. O SQLite recusou a operação: the query contains a null character.",
            status: 400,
        },
    },
    Caso {
        nome: "criar_erro_restricao_nul",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "A definição da tabela não é válida. O SQLite recusou a operação: the query contains a null character.",
            status: 400,
        },
    },
    Caso {
        nome: "criar_erro_nome_nul",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "A definição da tabela não é válida. O SQLite recusou a operação: the query contains a null character.",
            status: 400,
        },
    },
    Caso {
        nome: "criar_erro_coluna_nome_nul",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "A definição da tabela não é válida. O SQLite recusou a operação: the query contains a null character.",
            status: 400,
        },
    },
    Caso {
        nome: "criar_erro_padrao_nul",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "A definição da tabela não é válida. O SQLite recusou a operação: the query contains a null character.",
            status: 400,
        },
    },
    Caso {
        nome: "renomear_erro_nul",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "Não foi possível renomear a tabela. O SQLite recusou a operação: the query contains a null character.",
            status: 400,
        },
    },
    Caso {
        nome: "renomear_erro_origem_nul",
        setup: "",
        esperado: Esperado::Erro { mensagem: "A tabela clientes\0x não existe.", status: 404 },
    },
    Caso {
        nome: "excluir_tabela_erro_nul",
        setup: "",
        esperado: Esperado::Erro { mensagem: "A tabela eventos\0 não existe.", status: 404 },
    },
    Caso {
        nome: "alterar_erro_check_nul",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "A nova definição da tabela não é válida. O SQLite recusou a operação: the query contains a null character.",
            status: 400,
        },
    },
    Caso {
        nome: "alterar_erro_check_nul_cidade",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "A nova definição da tabela não é válida. O SQLite recusou a operação: the query contains a null character.",
            status: 400,
        },
    },
    Caso {
        nome: "alterar_erro_nome_nul",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "Não foi possível renomear a coluna. O SQLite recusou a operação: the query contains a null character.",
            status: 400,
        },
    },
    Caso {
        nome: "alterar_erro_tipo_nul",
        setup: "",
        esperado: Esperado::Erro { mensagem: "O tipo «TEXT\0» não é válido.", status: 400 },
    },
    Caso {
        nome: "alterar_erro_coluna_arg_nul",
        setup: "",
        esperado: Esperado::Erro { mensagem: "A coluna tipo\0 não existe em eventos.", status: 404 },
    },
    Caso {
        nome: "excluir_erro_coluna_nul",
        setup: "",
        esperado: Esperado::Erro { mensagem: "A coluna dados\0 não existe em eventos.", status: 404 },
    },
    Caso {
        nome: "excluir_erro_tabela_nul",
        setup: "",
        esperado: Esperado::Erro { mensagem: "A tabela eventos\0 não existe.", status: 404 },
    },
    Caso {
        nome: "reordenar_erro_nul",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "A nova ordem precisa ter exatamente as mesmas colunas da tabela, cada uma uma vez.",
            status: 400,
        },
    },
    Caso {
        nome: "indice_erro_nome_nul",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "Não foi possível criar o índice. O SQLite recusou a operação: the query contains a null character.",
            status: 400,
        },
    },
    Caso {
        nome: "indice_erro_coluna_nul",
        setup: "",
        esperado: Esperado::Erro { mensagem: "A coluna tipo\0 não existe em eventos.", status: 400 },
    },
    Caso {
        nome: "indice_erro_tabela_nul",
        setup: "",
        esperado: Esperado::Erro { mensagem: "A tabela eventos\0 não existe.", status: 404 },
    },
    Caso {
        nome: "excluir_indice_erro_nul",
        setup: "",
        esperado: Esperado::Erro { mensagem: "O índice idx_pedidos_cliente\0 não existe.", status: 404 },
    },
    Caso {
        nome: "visao_erro_nome_nul",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "Não foi possível criar a visão. O SQLite recusou a operação: the query contains a null character.",
            status: 400,
        },
    },
    Caso {
        nome: "visao_excluir_erro_nul",
        setup: "",
        esperado: Esperado::Erro { mensagem: "A visão v_pedidos_por_cliente\0 não existe.", status: 404 },
    },
    Caso {
        nome: "gatilho_excluir_erro_nul",
        setup: "",
        esperado: Esperado::Erro { mensagem: "O gatilho trg_pedido_cancelado\0 não existe.", status: 404 },
    },
    Caso {
        nome: "definicoes_nul",
        setup: "",
        esperado: Esperado::Erro { mensagem: "A tabela eventos\0 não existe.", status: 404 },
    },
    Caso {
        nome: "visao_erro_param",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "O SELECT da visão não é válido. O SQLite recusou a operação: Incorrect number of bindings supplied. The current statement uses 1, and there are 0 supplied..",
            status: 400,
        },
    },
    Caso {
        nome: "visao_erro_param_nome",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "O SELECT da visão não é válido. O SQLite recusou a operação: Incorrect number of bindings supplied. The current statement uses 1, and there are 0 supplied..",
            status: 400,
        },
    },
    Caso {
        nome: "visao_erro_param_dois",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "O SELECT da visão não é válido. O SQLite recusou a operação: Incorrect number of bindings supplied. The current statement uses 3, and there are 0 supplied..",
            status: 400,
        },
    },
    Caso {
        nome: "adicionar_erro_gerada_param",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "Não foi possível adicionar a coluna. O SQLite recusou a operação: Incorrect number of bindings supplied. The current statement uses 1, and there are 0 supplied..",
            status: 400,
        },
    },
    Caso {
        nome: "adicionar_erro_padrao_param",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "Não foi possível adicionar a coluna. O SQLite recusou a operação: default value of column [x] is not constant.",
            status: 400,
        },
    },
    Caso {
        nome: "criar_erro_check_param",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "A definição da tabela não é válida. O SQLite recusou a operação: parameters prohibited in CHECK constraints.",
            status: 400,
        },
    },
    Caso {
        nome: "indice_erro_onde_param",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "Não foi possível criar o índice. O SQLite recusou a operação: parameters prohibited in partial index WHERE clauses.",
            status: 400,
        },
    },
    Caso {
        nome: "gatilho_erro_param",
        setup: "",
        esperado: Esperado::Erro {
            mensagem: "Não foi possível criar o gatilho. O SQLite recusou a operação: trigger cannot use variables.",
            status: 400,
        },
    },
];

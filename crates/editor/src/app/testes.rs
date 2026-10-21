//! Os testes do aplicativo sem a janela: o Editor com o estado de um `Ambiente` e um `egui::Context` solto. Os
//! de ponta a ponta, com a janela num `egui_kittest::Harness`, ficam em `tests/`.

use banco::BancoInfo;
use comum::arquivos::Listagem;
use egui::ViewportCommand;
use estado::{Contexto, NoDaPasta, PastaAberta};
use interface::codigo::{AparenciaCodigo, CoresCodigo};
use interface::fontes;

use super::abrir::tem_extensao_de_banco;
use super::*;

#[test]
fn aparencia_das_prefs_vira_a_do_interface() {
    // as quatro cores pelo texto do arquivo (os dois enums têm de andar juntos)
    for (de_prefs, do_interface) in prefs::CoresCodigo::TODAS.into_iter().zip(CoresCodigo::TODAS) {
        assert_eq!(de_prefs.como_str(), do_interface.como_str());
        let a = prefs::Aparencia { cores_codigo: de_prefs, ..prefs::Aparencia::default() };
        assert_eq!(aparencia_da_interface(&a).cores, do_interface);
    }
    assert_eq!(aparencia_da_interface(&prefs::Aparencia::default()), AparenciaCodigo::default());
    assert_eq!(prefs::TAMANHO_CODIGO_PADRAO, interface::codigo::TAMANHO_PADRAO);
    let a = prefs::Aparencia {
        cores_codigo: prefs::CoresCodigo::Monokai,
        fonte_codigo: Some("Família que não existe".into()),
        tamanho_codigo: 14.0,
    };
    // aplicada: cores e tamanho valem; a família que não existe fica de fora
    let ctx = egui::Context::default();
    aplicar_aparencia(&ctx, &a);
    let atual = interface::codigo::atual(&ctx);
    assert_eq!(atual, AparenciaCodigo { cores: CoresCodigo::Monokai, familia: None, tamanho: 14.0 });
    assert_eq!(fontes::mono_escolhida(&ctx), None);
}

#[test]
fn erro_da_partida_como_janela_py() {
    let texto = erro_da_partida(std::path::Path::new(r"C:\dados\estragado.db"), "O arquivo não é um banco.");
    assert_eq!(
        texto,
        "Não foi possível abrir o banco C:\\dados\\estragado.db.\n\nO arquivo não é um banco.\n\nO DataBox abre \
         sem ele."
    );
}

#[test]
fn texto_da_troca_que_demora() {
    let abrir = Troca::Abrir { caminho: PathBuf::from("x.db"), somente_leitura: false };
    assert_eq!(abrir.em_andamento(false), "Abrindo o banco…");
    assert_eq!(Troca::Novo { caminho: PathBuf::from("x.db") }.em_andamento(false), "Criando o banco…");
    assert_eq!(Troca::Fechar.em_andamento(false), "Fechando o banco…");
    assert_eq!(abrir.em_andamento(true), "Abrindo o banco. Aguardando o banco atual terminar a operação em andamento.");
    assert!(Troca::Fechar.em_andamento(true).contains("operação em andamento"));
}

/// O pedido de fechar como o eframe o entrega ao `logic` (só o estado da janela conta no `run_logic`).
fn pedido_de_fechar_com(minimizada: bool, encoberta: bool) -> egui::RawInput {
    let mut raw = egui::RawInput::default();
    let info = raw.viewports.entry(egui::ViewportId::ROOT).or_default();
    info.minimized = Some(minimizada);
    info.occluded = Some(encoberta);
    info.events.push(egui::ViewportEvent::Close);
    raw
}

fn comandos_da_logica(e: &mut Editor, ctx: &egui::Context, raw: &egui::RawInput) -> Vec<ViewportCommand> {
    let saida = ctx.run_logic(raw, |c| e.logica(c));
    saida.viewport_commands.get(&egui::ViewportId::ROOT).cloned().unwrap_or_default()
}

/// O primeiro quadro da janela restaurada (sem pedido de fechar novo), só com o `pedido_de_fechar` no começo e o
/// `decidir_fechar_pendente` no fim: os comandos de fechar que eles mandaram (o primeiro quadro de um contexto
/// novo também manda o `SetTheme`).
fn comandos_do_quadro(e: &mut Editor, ctx: &egui::Context) -> Vec<ViewportCommand> {
    let saida = ctx.run_ui(egui::RawInput::default(), |ui| {
        e.pedido_de_fechar(ui.ctx());
        e.decidir_fechar_pendente(ui.ctx());
    });
    let comandos = saida.viewport_output.get(&egui::ViewportId::ROOT).map_or_else(Vec::new, |v| {
        v.commands
            .iter()
            .filter(|c| matches!(c, ViewportCommand::Close | ViewportCommand::CancelClose))
            .cloned()
            .collect()
    });
    // as texturas das fontes não vão a lugar nenhum aqui (sem pintura)
    saida.drop_without_applying_deltas();
    comandos
}

fn com_transacao(e: &mut Editor, aberta: bool) {
    if let Some(b) = e.estado.banco.as_mut() {
        b.transacao_aberta = aberta;
    }
}

/// O tipo do documento da aba (para conferir que a navegação o refez).
fn tipo_do_documento(e: &Editor) -> &'static str {
    match e.documento() {
        None => "nenhum",
        Some(Documento::Inicio(_)) => "inicio",
        Some(Documento::Banco(_)) => "banco",
        Some(Documento::Tabela(_)) => "tabela",
        Some(Documento::Consulta(_)) => "consulta",
        Some(Documento::Arquivo(_)) => "arquivo",
    }
}

/// O editor com o estado do ambiente já carregado, na Visão geral.
fn editor_pronto(amb: &estado::apoio_testes::Ambiente) -> Editor {
    let mut e = Editor::novo(Arc::clone(&amb.servico), Tema::Claro, Partida::default());
    e.estado = amb.estado.clone();
    e.carregado = true;
    e.reiniciar_abas(Rota::Banco, "");
    e
}

#[test]
fn navegar_abre_ou_foca_e_voltar_so_foca() {
    let amb = estado::apoio_testes::Ambiente::exemplo("abas_da_casca");
    let mut e = Editor::novo(Arc::clone(&amb.servico), Tema::Claro, Partida::default());
    assert_eq!(tipo_do_documento(&e), "nenhum", "antes de o estado chegar, nada à vista");
    assert!(!e.ocupado());
    e.estado = amb.estado.clone();
    e.carregado = true;
    // o erro do banco da linha de comando espera o primeiro Início
    e.erro_inicial = Some("não abriu".into());
    e.reiniciar_abas(Rota::Banco, "");
    assert_eq!(tipo_do_documento(&e), "banco");
    assert!(e.erro_inicial.is_some());
    let banco = e.abas.id_ativa();

    let clientes = Rota::objeto("clientes", false);
    e.navegar(clientes.clone());
    let tabela = e.abas.id_ativa();
    assert_ne!(tabela, banco);
    assert_eq!(tipo_do_documento(&e), "tabela");
    e.navegar(Rota::consulta_com("SELECT 1"));
    let consulta = e.abas.id_ativa();
    assert_eq!(tipo_do_documento(&e), "consulta");
    // o Substituir vai para a aba que o emitiu, mesmo fora de vista, sem refazer nada
    let pagina_2 = clientes.com_dados(estado::ConsultaDados { pagina: 2, ..Default::default() });
    e.trocar_endereco(Some(tabela), pagina_2.clone(), false);
    assert_eq!(e.abas.get(tabela).map(|a| a.rota()), Some(&pagina_2));
    assert_eq!(e.abas.id_ativa(), consulta);
    // um endereço que não é do documento da aba não vale
    e.trocar_endereco(Some(consulta), Rota::Banco, false);
    assert_eq!(e.rota(), &Rota::consulta_com("SELECT 1"));
    // voltar só foca a aba, com o endereço dela
    assert!(e.abas.voltar());
    assert_eq!((e.rota(), tipo_do_documento(&e)), (&pagina_2, "tabela"));
    // navegar até uma aba aberta a foca sem trocar o endereço
    e.navegar(Rota::Banco);
    e.navegar(clientes.clone());
    assert_eq!((e.abas.id_ativa(), e.rota()), (tabela, &pagina_2));
    assert_eq!(e.abas.len(), 3);

    e.navegar(Rota::Inicio);
    assert_eq!(tipo_do_documento(&e), "inicio");
    assert!(e.erro_inicial.is_none(), "o Início levou o erro");
    // fechar a ativa vai para a usada antes
    e.fechar_aba(e.abas.id_ativa());
    assert_eq!(e.abas.id_ativa(), tabela);
    assert_eq!(e.abas.len(), 3);

    // a troca de banco recomeça as abas (outros ids) e o histórico
    e.reiniciar_abas(Rota::Banco, "");
    assert_eq!(e.abas.len(), 1);
    assert!(e.abas.id_ativa() > consulta);
    assert!(!e.abas.pode_voltar());
    assert_eq!(e.rota(), &Rota::Banco);
    // sem banco, toda rota volta ao Início, também a Nova consulta
    e.estado.banco = None;
    e.navegar(clientes);
    assert_eq!((e.rota(), tipo_do_documento(&e)), (&Rota::Inicio, "inicio"));
    e.nova_consulta(Some("SELECT 1".into()));
    assert_eq!((e.rota(), e.abas.len()), (&Rota::Inicio, 2));
}

#[test]
fn a_transacao_lembra_a_consulta_que_a_abriu() {
    let amb = estado::apoio_testes::Ambiente::exemplo("consulta_da_transacao");
    let mut e = editor_pronto(&amb);
    let ctx = egui::Context::default();
    e.nova_consulta(None);
    let c1 = e.abas.id_ativa();
    e.nova_consulta(None);
    let c2 = e.abas.id_ativa();
    // uma aba de tabela que grava dentro da transação não é a da transação
    e.navegar(Rota::objeto("clientes", false));
    let tabela = e.abas.id_ativa();
    e.notar_transacao(&ctx, Some(tabela), true);
    assert!(e.estado.transacao_aberta());
    assert_eq!(e.consulta_da_transacao, None);
    e.notar_transacao(&ctx, None, false);

    // o BEGIN na 1; a sondagem da 2 vê a mesma transação, que continua sendo da 1
    e.notar_transacao(&ctx, Some(c1), true);
    e.notar_transacao(&ctx, Some(c2), true);
    assert_eq!(e.consulta_da_transacao, Some(c1));
    e.navegar(Rota::Consulta { sql: None });
    assert_eq!(e.abas.id_ativa(), c1, "o \"Ir para a Consulta SQL\" foca a da transação");
    // fechada a 1, a mais recente; e a primeira Consulta que vê a transação aberta passa a ser a dela
    e.fechar_aba(c1);
    e.navegar(Rota::objeto("clientes", false));
    e.navegar(Rota::Consulta { sql: None });
    assert_eq!(e.abas.id_ativa(), c2);
    e.notar_transacao(&ctx, Some(c2), true);
    assert_eq!(e.consulta_da_transacao, Some(c2));
    // o COMMIT (de onde vier) tira a transação
    e.notar_transacao(&ctx, None, false);
    assert_eq!(e.consulta_da_transacao, None);
    assert!(!e.estado.transacao_aberta());
}

/// C14: a leitura do BancoInfo que volta atrasada não desfaz a transação que a nota seguinte acertou, nem uma
/// leitura mais nova já aplicada.
#[test]
fn leitura_atrasada_do_banco_nao_desfaz_a_transacao_nem_a_leitura_mais_nova() {
    let amb = estado::apoio_testes::Ambiente::exemplo("leitura_atrasada");
    let mut e = editor_pronto(&amb);
    let ctx = egui::Context::default();
    let id = e.estado.banco_id();
    let base = e.estado.banco.clone().expect("banco");
    let info = |aberta: bool, tamanho: u64| BancoInfo { transacao_aberta: aberta, tamanho, ..base.clone() };
    // um BEGIN e um COMMIT seguidos: duas notas que mudam, duas leituras no ar (0 e 1)
    e.notar_transacao(&ctx, None, true);
    e.notar_transacao(&ctx, None, false);
    assert!(!e.estado.transacao_aberta());
    // a leitura 0, do tempo do BEGIN, chega depois do COMMIT: o resto vale, a transação não volta
    e.fim_da_tarefa(&ctx, FimTarefa::Banco(id.clone(), 0, Ok(info(true, 1234))));
    assert!(!e.estado.transacao_aberta(), "a leitura velha não reabre a transação");
    assert_eq!(e.estado.banco.as_ref().map(|b| b.tamanho), Some(1234));
    // a leitura 1, de depois do COMMIT, vale inteira; a 0 que chegue de novo é mais velha que a aplicada
    e.fim_da_tarefa(&ctx, FimTarefa::Banco(id.clone(), 1, Ok(info(false, 5678))));
    assert_eq!(e.estado.banco.as_ref().map(|b| b.tamanho), Some(5678));
    e.fim_da_tarefa(&ctx, FimTarefa::Banco(id, 0, Ok(info(true, 1))));
    assert_eq!(e.estado.banco.as_ref().map(|b| b.tamanho), Some(5678), "a mais velha é descartada");
    assert!(!e.estado.transacao_aberta());
}

/// C13: limpar o histórico só marca a lista como velha quando a gravação (que limpa o arquivo) termina: antes, a
/// releitura corria contra a limpeza e podia trazer a lista de antes.
#[test]
fn limpar_o_historico_marca_a_lista_velha_so_quando_a_gravacao_termina() {
    let amb = estado::apoio_testes::Ambiente::exemplo("limpar_historico");
    let mut e = editor_pronto(&amb);
    let ctx = egui::Context::default();
    e.casca.historico = Default::default();
    assert!(!e.casca.historico.velha());
    let promessa = Promessa::rodar(&ctx, {
        let prefs = e.estado.prefs.clone();
        move || {
            std::thread::sleep(std::time::Duration::from_millis(300));
            Ok(prefs)
        }
    });
    e.tarefas.push(Tarefa::Tema { promessa, limpou_historico: true });
    e.receber(&ctx);
    assert!(!e.casca.historico.velha(), "a gravação ainda não terminou");
    let inicio = std::time::Instant::now();
    while !e.casca.historico.velha() {
        assert!(inicio.elapsed() < std::time::Duration::from_secs(10), "a gravação não terminou");
        std::thread::sleep(std::time::Duration::from_millis(20));
        e.receber(&ctx);
    }
    assert!(e.tarefas.is_empty());
}

#[test]
fn objetos_que_sumiram_fecham_as_abas_menos_a_ativa() {
    let amb = estado::apoio_testes::Ambiente::exemplo("objetos_sumidos");
    let mut e = editor_pronto(&amb);
    e.navegar(Rota::objeto("clientes", false));
    // a mesma aba (uma aba por objeto): a parte vira a vista das Propriedades
    e.navegar(Rota::objeto_na_aba("clientes", false, estado::Parte::Estrutura));
    e.navegar(Rota::objeto("produtos", false));
    e.navegar(Rota::objeto("pedidos", false));
    let pedidos = e.abas.id_ativa();
    e.navegar(Rota::objeto("fantasma", false));
    let fantasma = e.abas.id_ativa();
    e.navegar(Rota::objeto("pedidos", false));
    let mut objetos = e.estado.objetos.clone().expect("objetos");
    let ctx = egui::Context::default();
    // a lista igual não fecha nada (nem a aba de um nome que nunca existiu)
    e.trocar_objetos(&ctx, objetos.clone());
    assert_eq!(e.abas.len(), 5);
    objetos.tabelas.retain(|t| t.nome != "clientes" && t.nome != "produtos" && t.nome != "pedidos");
    e.trocar_objetos(&ctx, objetos);
    let ids: Vec<IdAba> = e.abas.iter().map(|a| a.id()).collect();
    assert_eq!(ids.len(), 3, "a de clientes e a de produtos fecharam");
    assert!(ids.contains(&pedidos), "a ativa fica, com o card de que não existe mais");
    assert!(ids.contains(&fantasma));
}

#[test]
fn recarregar_objetos_de_outra_aba_rele_tambem_a_ativa_no_quadro_seguinte() {
    // antes, a ativa ficava como estava: a grade à vista mostrava as linhas de antes de um INSERT da Consulta
    // escondida ou da execução solta
    let amb = estado::apoio_testes::Ambiente::exemplo("reler_a_ativa");
    let mut e = editor_pronto(&amb);
    let ctx = egui::Context::default();
    e.nova_consulta(None);
    let consulta = e.abas.id_ativa();
    e.navegar(Rota::objeto("clientes", false));
    let tabela = e.abas.id_ativa();
    // da Consulta escondida: a ativa relê no fim do quadro seguinte (depois de a célula em edição gravar)
    e.marcar_para_reler(&ctx, Some(consulta));
    assert_eq!(e.reler_ativa, Some(tabela));
    // da execução solta (sem aba) também
    e.reler_ativa = None;
    e.marcar_para_reler(&ctx, None);
    assert_eq!(e.reler_ativa, Some(tabela));
    // a aba de origem não se marca
    e.reler_ativa = None;
    e.marcar_para_reler(&ctx, Some(tabela));
    assert_eq!(e.reler_ativa, None);
}

#[test]
fn execucao_solta_so_recarrega_os_objetos_quando_mudou_algo() {
    let amb = estado::apoio_testes::Ambiente::exemplo("solta_sem_mudanca");
    let mut e = editor_pronto(&amb);
    let ctx = egui::Context::default();
    let id = e.estado.banco_id();
    let resposta = |dados: bool, esquema: bool, aberta: bool| banco::ResultadoConsulta {
        resultados: Vec::new(),
        alterou_esquema: esquema,
        alterou_dados: dados,
        transacao_aberta: aberta,
    };
    let recarregou = |e: &mut Editor| {
        let sim = e.pendentes.iter().any(|(_, a)| matches!(a, Acao::RecarregarObjetos));
        e.pendentes.clear();
        sim
    };
    // um SELECT: só a transação
    e.fim_da_tarefa(&ctx, FimTarefa::Consulta(id.clone(), Ok(resposta(false, false, false))));
    assert!(!recarregou(&mut e));
    // um INSERT ou um CREATE: os objetos e as abas
    e.fim_da_tarefa(&ctx, FimTarefa::Consulta(id.clone(), Ok(resposta(true, false, false))));
    assert!(recarregou(&mut e));
    e.fim_da_tarefa(&ctx, FimTarefa::Consulta(id.clone(), Ok(resposta(false, true, false))));
    assert!(recarregou(&mut e));
    // o ROLLBACK de uma transação aberta (os dados voltaram)
    com_transacao(&mut e, true);
    e.fim_da_tarefa(&ctx, FimTarefa::Consulta(id, Ok(resposta(false, false, false))));
    assert!(recarregou(&mut e));
}

#[test]
fn logica_com_a_janela_minimizada_cancela_e_restaura() {
    let amb = estado::apoio_testes::Ambiente::exemplo("logica_minimizada");
    let mut e = Editor::novo(Arc::clone(&amb.servico), Tema::Claro, Partida::default());
    e.estado = amb.estado.clone();
    let ctx = egui::Context::default();
    let minimizada = pedido_de_fechar_com(true, false);
    let restaurar = vec![ViewportCommand::CancelClose, ViewportCommand::Minimized(false), ViewportCommand::Focus];

    // sem exportação nem transação, fecha como antes, sem perguntar
    assert_eq!(comandos_da_logica(&mut e, &ctx, &minimizada), vec![]);
    assert!(!e.fechar_pendente);

    com_transacao(&mut e, true);
    // à vista, quem pergunta é o quadro
    assert_eq!(comandos_da_logica(&mut e, &ctx, &pedido_de_fechar_com(false, false)), vec![]);
    assert!(!e.fechar_pendente);

    // minimizada, com a transação aberta: não fecha, volta à vista e pergunta no quadro seguinte (sem outro
    // CancelClose: o pedido já foi cancelado)
    assert_eq!(comandos_da_logica(&mut e, &ctx, &minimizada), restaurar);
    assert!(e.fechar_pendente);
    assert_eq!(comandos_do_quadro(&mut e, &ctx), vec![]);
    assert!(!e.fechar_pendente);
    assert!(e.confirmacao.is_some());
    assert!(matches!(e.confirmado, Some(Confirmado::FecharApp { falta_transacao: false })));
    e.confirmacao = None;
    e.confirmado = None;

    // encoberta, o mesmo; se a transação acabou enquanto a janela voltava, ela fecha, como a pessoa pediu
    assert_eq!(comandos_da_logica(&mut e, &ctx, &pedido_de_fechar_com(false, true)), restaurar);
    assert!(e.fechar_pendente);
    com_transacao(&mut e, false);
    assert_eq!(comandos_do_quadro(&mut e, &ctx), vec![ViewportCommand::Close]);
    assert!(e.confirmacao.is_none());
    com_transacao(&mut e, true);

    // sem o pedido de fechar, nada
    let mut so_minimizada = minimizada.clone();
    if let Some(info) = so_minimizada.viewports.get_mut(&egui::ViewportId::ROOT) {
        info.events.clear();
    }
    assert_eq!(comandos_da_logica(&mut e, &ctx, &so_minimizada), vec![]);
    assert!(!e.fechar_pendente);

    // depois do "Descartar e continuar", fecha sem perguntar de novo
    e.pode_fechar = true;
    assert_eq!(comandos_da_logica(&mut e, &ctx, &minimizada), vec![]);
    assert!(!e.fechar_pendente);
}

#[test]
fn logica_restaura_com_a_resposta_do_begin_ainda_nao_tomada() {
    // antes, minimizada, a casca decidia pelo estado de antes da resposta (sem transação) e fechava sem perguntar
    let amb = estado::apoio_testes::Ambiente::exemplo("logica_begin");
    let mut e = Editor::novo(Arc::clone(&amb.servico), Tema::Claro, Partida::default());
    e.estado = amb.estado.clone();
    let ctx = egui::Context::default();
    let minimizada = pedido_de_fechar_com(true, false);
    let restaurar = vec![ViewportCommand::CancelClose, ViewportCommand::Minimized(false), ViewportCommand::Focus];
    // a execução solta de uma Consulta fechada, com a resposta do BEGIN já pronta (só um quadro a toma)
    let resposta = banco::ResultadoConsulta {
        resultados: Vec::new(),
        alterou_esquema: false,
        alterou_dados: false,
        transacao_aberta: true,
    };
    e.tarefas.push(Tarefa::Consulta(e.estado.banco_id(), Promessa::pronta(Ok(resposta))));
    assert_eq!(comandos_da_logica(&mut e, &ctx, &minimizada), restaurar);
    assert!(e.fechar_pendente);
    // o primeiro quadro da janela restaurada toma a resposta e só depois decide: pergunta da transação
    let saida = ctx.run_ui(egui::RawInput::default(), |ui| {
        let c = ui.ctx();
        e.pedido_de_fechar(c);
        e.receber(c);
        let acoes = std::mem::take(&mut e.pendentes);
        e.aplicar(c, None, acoes);
        e.decidir_fechar_pendente(c);
    });
    saida.drop_without_applying_deltas();
    assert!(e.estado.transacao_aberta());
    assert!(matches!(e.confirmado, Some(Confirmado::FecharApp { falta_transacao: false })));
    assert!(e.confirmacao.is_some());
}

/// A leitura de uma pasta que volta depois de outra pasta abrir é descartada; a da pasta aberta entra, e a mesma
/// listagem de novo não muda o estado (a versão fica: nada repinta).
#[test]
fn leitura_de_outra_pasta_e_descartada() {
    let amb = estado::apoio_testes::Ambiente::vazio("leitura_de_outra_pasta");
    let mut e = editor_pronto(&amb);
    let ctx = egui::Context::default();
    let a = PathBuf::from(r"C:\dados\a");
    let b = PathBuf::from(r"C:\dados\b");
    e.estado.pasta = Some(PastaAberta::nova(b.clone()));
    let chave_a = prefs::chave_caminho(&a);
    let chave_b = prefs::chave_caminho(&b);
    let lida = |p: &Path| Listagem::montar(p.to_path_buf(), Vec::new(), 0, false, 500);
    e.chegou_a_pasta(&ctx, &chave_a, chave_a.clone(), Ok(lida(&a)));
    let p = e.estado.pasta.as_ref().expect("pasta");
    assert_eq!(p.chave, chave_b, "a pasta aberta continua a b");
    assert_eq!(p.nos.get(&chave_b), Some(&NoDaPasta::Lendo), "nada da a entrou");
    assert!(!p.nos.contains_key(&chave_a));
    e.chegou_a_pasta(&ctx, &chave_b, chave_b.clone(), Ok(lida(&b)));
    assert!(e.estado.pasta.as_ref().and_then(|p| p.listagem(&chave_b)).is_some());
    let versao = e.versao;
    e.chegou_a_pasta(&ctx, &chave_b, chave_b.clone(), Ok(lida(&b)));
    assert_eq!(e.versao, versao, "a mesma listagem não muda o estado");
    // a falha de uma subpasta fica no nó dela
    let sub = prefs::chave_caminho(b.join("sub"));
    e.chegou_a_pasta(&ctx, &chave_b, sub.clone(), Err(ErroBanco::novo("sem acesso")));
    assert_eq!(e.estado.pasta.as_ref().and_then(|p| p.nos.get(&sub)), Some(&NoDaPasta::Falhou("sem acesso".into())));
    assert!(e.versao > versao);
}

#[test]
fn textos_da_partida_da_pasta_e_do_arquivo() {
    assert_eq!(
        erro_da_partida_pasta(Path::new(r"C:\dados"), "A pasta não existe (ou foi movida): C:\\dados."),
        "Não foi possível abrir a pasta C:\\dados.\n\nA pasta não existe (ou foi movida): C:\\dados.\n\nO DataBox \
         abre sem ela."
    );
    assert!(erro_da_partida_arquivo(Path::new(r"C:\a.csv"), "x").starts_with("Não foi possível abrir o arquivo"));
    assert!(tem_extensao_de_banco(Path::new(r"C:\x\vendas.DB")) && tem_extensao_de_banco(Path::new("a.sqlite3")));
    assert!(!tem_extensao_de_banco(Path::new("a.csv")) && !tem_extensao_de_banco(Path::new("sem_extensao")));
}

#[test]
fn pergunta_da_fila_que_some_sem_resposta_para_a_fila() {
    // a pergunta "Salvar as alterações em…?" que outra confirmação apaga (o banco trocado por fora fecha as
    // janelas) não vale como "Não salvar": a fila para e a aba alterada continua aberta
    struct Alterado;
    impl estado::Documento for Alterado {
        fn conteudo(&mut self, _ui: &mut egui::Ui, _ctx: &mut Contexto) {}
        fn alterado(&self) -> bool {
            true
        }
    }
    let amb = estado::apoio_testes::Ambiente::vazio("fila_sem_resposta");
    let mut e = Editor::novo(Arc::clone(&amb.servico), Tema::Claro, Partida::default());
    let ctx = egui::Context::default();
    let caminho = amb.pasta().join("vendas.csv");
    let id = e.abas.abrir(Rota::arquivo(&caminho), |_, _| Documento::de_arquivo_com(&caminho, Box::new(Alterado))).id();
    e.pedir_fechar_aba(&ctx, id);
    e.andar_fila(&ctx);
    assert!(matches!(e.confirmado, Some(Confirmado::SalvarAlterado(p)) if p == id), "a pergunta da aba");
    e.confirmacao = None;
    e.confirmado = None;
    e.andar_fila(&ctx);
    assert!(e.fila.is_none(), "a fila parou");
    assert!(e.abas.get(id).is_some(), "a aba alterada continua aberta");
}

//! DataBox.exe:
//!
//! ```text
//! DataBox.exe [caminho] [--somente-leitura] [--renderizador glow|wgpu] [--criar-atalho]
//! ```
//!
//! Liga o registro (com o gancho de pânico), abre as preferências e o serviço, e abre a janela com o tema das preferências e o movimento reduzido do Windows.
//! No Windows, a janela fica sem a decoração do sistema, com a barra de título própria ([`BARRA_PROPRIA`]).
//! O caminho da linha de comando (uma pasta, um banco ou um arquivo de dados) é aberto no começo (se não abrir, o
//! aplicativo abre sem ele e mostra o motivo no Início). O renderizador padrão é o glow (OpenGL); se ele não abrir (máquina virtual sem driver), o executável
//! se relança com `--renderizador wgpu` e sai. Sem console: ajuda, versão, erros de argumento e falhas fatais
//! aparecem numa caixa de mensagem.

#![windows_subsystem = "windows"]
#![deny(unsafe_code)]

use std::ffi::OsString;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use comum::app::NOME;
use editor::linha_comando::{self, Argumentos, Comando, Renderizador};
use editor::{Editor, Partida, atalho, registro, tema_da_interface};

/// Tamanho inicial e mínimo da janela: o inicial deixa as Propriedades à vista, na proporção 16:10.
const TAMANHO_INICIAL: [f32; 2] = [1280.0, 800.0];
const TAMANHO_MINIMO: [f32; 2] = [960.0, 600.0];
/// No Windows, a janela fica sem a decoração do sistema e a barra de título é a do DataBox (os menus, o título e os
/// botões minimizar, maximizar e fechar numa faixa só; o app cuida do arrasto e das bordas). Fora dele, a barra do
/// sistema continua, e a faixa fica só com o logo, os menus e o título.
const BARRA_PROPRIA: bool = cfg!(windows);

/// Caixa de mensagem do Windows (sem console é o único jeito de a pessoa ver o que houve).
fn caixa(texto: &str, erro: bool) {
    // fora do Windows o programa costuma ser chamado de um terminal, e o diálogo pode não existir (sem portal)
    #[cfg(not(windows))]
    if erro {
        eprintln!("{texto}");
    } else {
        println!("{texto}");
    }
    let nivel = if erro { rfd::MessageLevel::Error } else { rfd::MessageLevel::Info };
    let _ = rfd::MessageDialog::new()
        .set_level(nivel)
        .set_title(NOME)
        .set_description(texto)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
}

/// A frase de uma falha fatal, com o caminho do registro.
fn falha_fatal(frase: &str) -> String {
    format!(
        "O {NOME} não conseguiu continuar.\n\n{frase}\n\nOs detalhes ficaram no arquivo de log:\n{}",
        servico::arquivo_log().display()
    )
}

fn main() -> ExitCode {
    let argv: Vec<OsString> = std::env::args_os().skip(1).collect();
    let args = match linha_comando::analisar(&argv) {
        Ok(Comando::Executar(args)) => args,
        Ok(Comando::Ajuda) => {
            caixa(linha_comando::AJUDA, false);
            return ExitCode::SUCCESS;
        }
        Ok(Comando::Versao) => {
            caixa(&format!("{NOME} {}", servico::versao()), false);
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            caixa(&e.texto(), true);
            return ExitCode::from(2);
        }
    };
    registro::iniciar(servico::arquivo_log());
    log::info!("{NOME} {} iniciando (renderizador {}).", servico::versao(), args.renderizador.como_str());
    let codigo = if args.criar_atalho { criar_atalho() } else { executar(&args) };
    log::info!("{NOME} encerrado.");
    codigo
}

/// `--criar-atalho`: os atalhos na Área de Trabalho e no Menu Iniciar, para o próprio executável.
fn criar_atalho() -> ExitCode {
    let exe = match std::env::current_exe() {
        Ok(exe) => atalho::executavel_do_pacote(std::env::var_os("APPIMAGE"), exe),
        Err(e) => {
            log::error!("Não foi possível saber onde está o executável: {e}");
            caixa(&falha_fatal(&format!("Não foi possível saber onde está o executável: {e}")), true);
            return ExitCode::FAILURE;
        }
    };
    match atalho::criar_atalhos(&exe, None) {
        Ok(criados) => {
            let linhas: Vec<String> = criados.iter().map(|l| format!("Atalho criado: {}", l.display())).collect();
            for l in &linhas {
                log::info!("{l}");
            }
            caixa(&linhas.join("\n"), false);
            ExitCode::SUCCESS
        }
        Err(e) => {
            log::error!("{e}");
            caixa(&falha_fatal(&e), true);
            ExitCode::FAILURE
        }
    }
}

fn opcoes_nativas(renderizador: Renderizador) -> eframe::NativeOptions {
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(NOME)
        // sem o recurso `persistence` do eframe, o app_id só vale no Wayland (o nome da janela), para o porte futuro
        .with_app_id(NOME)
        .with_inner_size(TAMANHO_INICIAL)
        .with_min_inner_size(TAMANHO_MINIMO);
    if let Some(icone) = interface::marca::icone_janela() {
        viewport = viewport.with_icon(Arc::new(icone));
    }
    if BARRA_PROPRIA {
        // o winit mantém os estilos de legenda e de borda (o Aero Snap, o Win+setas e o Alt+F4 continuam) e o
        // egui-winit pede a sombra da janela sem decoração
        viewport = viewport.with_decorations(false);
    }
    eframe::NativeOptions {
        viewport,
        renderer: match renderizador {
            Renderizador::Glow => eframe::Renderer::Glow,
            Renderizador::Wgpu => eframe::Renderer::Wgpu,
        },
        centered: true,
        persist_window: false,
        ..Default::default()
    }
}

fn executar(args: &Argumentos) -> ExitCode {
    let prefs = prefs::Preferencias::abrir(None);
    let tema = tema_da_interface(prefs.tema());
    let aparencia = prefs.aparencia();
    let paineis = prefs.paineis();
    let lugares = prefs.lugares();
    let servico = Arc::new(servico::Servico::novo(prefs));
    let servico_ao_sair = Arc::clone(&servico);
    let reduzido = movimento_reduzido_do_windows();
    let partida = Partida { caminho: args.caminho.clone(), somente_leitura: args.somente_leitura };
    let renderizador = args.renderizador;
    let criado = Arc::new(AtomicBool::new(false));
    let marcador = Arc::clone(&criado);
    let resultado = eframe::run_native(
        NOME,
        opcoes_nativas(args.renderizador),
        Box::new(move |cc| {
            marcador.store(true, Ordering::SeqCst);
            // o tema das preferências vale antes do primeiro quadro (a janela não pisca clara)
            interface::tema::aplicar(&cc.egui_ctx, tema);
            // e a aparência do código (cores, família e tamanho), também antes do primeiro quadro
            editor::aplicar_aparencia(&cc.egui_ctx, &aparencia);
            interface::tema::definir_movimento_reduzido(&cc.egui_ctx, reduzido);
            // o vidro fosco: o que está atrás dos menus, janelas e avisos sai desfocado (a foto da própria janela)
            interface::fosco::ligar(&cc.egui_ctx, Some(|linha| log::info!("{linha}")));
            // a borda de 1px, a linha do topo e a sombra do DWM seguem o tema do app: a cada mudança do tema, o egui
            // manda o `ViewportCommand::SetTheme` (Sistema → o do Windows, Claro, Escuro)
            cc.egui_ctx.options_mut(|o| o.sync_window_theme = true);
            // o renderizador que abriu a janela, para a janela "Sobre o DataBox"
            // e as larguras das docas que a pessoa deixou (as divisórias) e onde ela pôs cada painel
            Ok(Box::new(
                Editor::novo(servico, tema, partida)
                    .com_barra_propria(BARRA_PROPRIA)
                    .com_renderizador(renderizador)
                    .com_larguras(paineis)
                    .com_lugares(lugares),
            ))
        }),
    );
    // A janela já fechou (e, com ela, a janela Exportar, que pede para interromper ao sair): as tarefas de fundo
    // terminam antes do processo — uma gravação em voo vai até o fim e a exportação interrompida apaga o
    // temporário dela (na web, o servidor seguia vivo alguns segundos depois de a janela fechar). Uma Consulta SQL
    // ainda rodando é cortada antes (a pessoa já mandou fechar), em vez de esperar o prazo e morrer no meio.
    if editor::interromper_ao_sair(&servico_ao_sair) {
        log::info!("Fechando com uma Consulta SQL em andamento: interrompida.");
    }
    if !estado::esperar_tarefas(std::time::Duration::from_secs(5)) {
        log::warn!("Fechando com {} tarefa(s) de fundo ainda em andamento.", estado::tarefas_em_andamento());
    }
    match resultado {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) if args.renderizador == Renderizador::Glow && !criado.load(Ordering::SeqCst) => {
            log::warn!("O OpenGL não abriu ({e}); o {NOME} se abre de novo com wgpu.");
            relancar(args)
        }
        Err(e) => {
            log::error!("Falha fatal: a janela não abriu: {e}");
            caixa(&falha_fatal(&format!("A janela não pôde ser aberta: {e}")), true);
            ExitCode::FAILURE
        }
    }
}

/// Abre o próprio executável de novo com `--renderizador wgpu` (os mesmos argumentos) e sai.
fn relancar(args: &Argumentos) -> ExitCode {
    let novos = linha_comando::argumentos_para_relancar(args, Renderizador::Wgpu);
    let lancado = std::env::current_exe()
        .map(|exe| atalho::executavel_do_pacote(std::env::var_os("APPIMAGE"), exe))
        .and_then(|exe| std::process::Command::new(exe).args(&novos).spawn());
    match lancado {
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            log::error!("Não foi possível abrir o {NOME} de novo com wgpu: {e}");
            caixa(&falha_fatal(&format!("A janela não pôde ser aberta (nem com OpenGL nem com wgpu): {e}")), true);
            ExitCode::FAILURE
        }
    }
}

/// O "Mostrar animações no Windows" desligado (Configurações › Acessibilidade › Efeitos visuais) vira o
/// movimento reduzido da interface.
#[cfg(windows)]
#[allow(unsafe_code)]
fn movimento_reduzido_do_windows() -> bool {
    const SPI_GETCLIENTAREAANIMATION: u32 = 0x1042;
    #[link(name = "user32")]
    unsafe extern "system" {
        fn SystemParametersInfoW(acao: u32, parametro: u32, dados: *mut core::ffi::c_void, ini: u32) -> i32;
    }
    let mut animacao: i32 = 1;
    // SAFETY: SPI_GETCLIENTAREAANIMATION escreve um BOOL (i32) no endereço passado, que aponta para `animacao`,
    // viva e alinhada durante toda a chamada; os outros parâmetros são zero, como a documentação pede.
    let ok = unsafe { SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, (&mut animacao as *mut i32).cast(), 0) };
    ok != 0 && animacao == 0
}

/// Fora do Windows ainda não se lê a preferência do sistema: as animações ficam ligadas.
#[cfg(not(windows))]
fn movimento_reduzido_do_windows() -> bool {
    false
}

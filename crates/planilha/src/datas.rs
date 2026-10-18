//! Datas do Excel: o número de série (dias desde uma origem, com a hora na parte fracionária) e o texto ISO
//! das células `t="d"`.
//!
//! - Sistema 1900 (o normal): a série 1 é 1900-01-01 e a 60 é 1900-02-29, dia que não existiu (o Excel herdou
//!   o erro do Lotus 1-2-3). Daí em diante (série 61 = 1900-03-01) é o calendário de verdade. A série 0 é
//!   "1900-01-00", como o Excel mostra.
//! - Sistema 1904 (`date1904` no livro; Excel antigo de Mac): a série 0 é 1904-01-01 e não há dia fantasma.
//! - A hora é arredondada ao milissegundo (uma série como 0,999999999 vira meia-noite do dia seguinte).

use crate::valores::{Data, Hora, Momento, Valor};

/// A origem das séries do livro (`<workbookPr date1904>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SistemaDatas {
    #[default]
    D1900,
    D1904,
}

const MS_POR_DIA: i64 = 86_400_000;
/// Séries do sistema 1900 entre 1904-01-01 (série 1462) e a origem do 1904 (série 0).
const DIFERENCA_1904: i64 = 1462;
/// Série (sistema 1900) de 1970-01-01.
const SERIE_1970: i64 = 25_569;
/// Última data do Excel: 9999-12-31 (série 2.958.465 no sistema 1900).
const SERIE_MAXIMA: i64 = 2_958_465;

/// Dias desde 1970-01-01 → data (algoritmo de Howard Hinnant).
pub(crate) fn data_de_dias(dias: i64) -> Data {
    let z = dias + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let dia = (doy - (153 * mp + 2) / 5 + 1) as u8;
    let mes = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
    let ano = (yoe + era * 400 + i64::from(mes <= 2)) as i32;
    Data { ano, mes, dia }
}

/// Data → dias desde 1970-01-01 (o inverso de [`data_de_dias`]).
pub(crate) fn dias_de_data(d: Data) -> i64 {
    let ano = i64::from(d.ano) - i64::from(d.mes <= 2);
    let era = ano.div_euclid(400);
    let yoe = ano.rem_euclid(400);
    let mes = i64::from(d.mes);
    let doy = (153 * (if mes > 2 { mes - 3 } else { mes + 9 }) + 2) / 5 + i64::from(d.dia) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

pub(crate) fn dias_no_mes(ano: i32, mes: u8) -> u8 {
    match mes {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if (ano % 4 == 0 && ano % 100 != 0) || ano % 400 == 0 {
                29
            } else {
                28
            }
        }
    }
}

/// Dia da semana como o Excel conta (0 = domingo). Nas datas anteriores a 1900-03-01 o Excel está errado
/// por um dia (trata 1900-01-01 como domingo, e foi segunda); aqui vale o do Excel, que é o que se vê nele.
pub(crate) fn dia_da_semana(d: Data) -> u8 {
    if (d.ano, d.mes) < (1900, 3) {
        let serie = if d.mes == 1 { i64::from(d.dia) } else { 31 + i64::from(d.dia) };
        return ((serie + 6).rem_euclid(7)) as u8;
    }
    // 1970-01-01 foi quinta-feira
    ((dias_de_data(d) + 4).rem_euclid(7)) as u8
}

/// Série → data e hora. `None` para série negativa, fora do calendário ou que não é número.
pub fn serial_para_momento(serial: f64, sistema: SistemaDatas) -> Option<Momento> {
    if !serial.is_finite() || serial < 0.0 || serial > 3_000_000.0 {
        return None;
    }
    let total_ms = (serial * MS_POR_DIA as f64).round() as i64;
    let dias = total_ms.div_euclid(MS_POR_DIA);
    let ms = total_ms.rem_euclid(MS_POR_DIA);
    let dias_1900 = match sistema {
        SistemaDatas::D1900 => dias,
        SistemaDatas::D1904 => dias + DIFERENCA_1904,
    };
    if dias_1900 > SERIE_MAXIMA {
        return None;
    }
    let data = match dias_1900 {
        0 => Data { ano: 1900, mes: 1, dia: 0 },
        1..=31 => Data { ano: 1900, mes: 1, dia: dias_1900 as u8 },
        32..=59 => Data { ano: 1900, mes: 2, dia: (dias_1900 - 31) as u8 },
        60 => Data { ano: 1900, mes: 2, dia: 29 },
        _ => data_de_dias(dias_1900 - SERIE_1970),
    };
    Some(Momento { data, hora: hora_de_ms(ms) })
}

/// Milissegundos do dia → hora.
fn hora_de_ms(ms: i64) -> Hora {
    Hora {
        hora: (ms / 3_600_000) as u8,
        minuto: (ms / 60_000 % 60) as u8,
        segundo: (ms / 1000 % 60) as u8,
        milissegundo: (ms % 1000) as u16,
    }
}

/// Só a parte da hora de uma série (para formatos de hora, onde 1,5 e 0,5 aparecem iguais). `None` se negativa.
pub fn serial_para_hora(serial: f64) -> Option<Hora> {
    if !serial.is_finite() || serial < 0.0 || serial > 3_000_000.0 {
        return None;
    }
    let total_ms = (serial * MS_POR_DIA as f64).round() as i64;
    Some(hora_de_ms(total_ms.rem_euclid(MS_POR_DIA)))
}

/// A série como a categoria do formato pede; se não couber no calendário, continua número.
pub(crate) fn valor_de_serie(serie: f64, so_data: bool, so_hora: bool, sistema: SistemaDatas) -> Valor {
    let momento = if so_hora { None } else { serial_para_momento(serie, sistema) };
    match (so_hora, so_data, momento) {
        (true, _, _) => serial_para_hora(serie).map_or(Valor::Numero(serie), Valor::Hora),
        (false, true, Some(m)) => Valor::Data(m.data),
        (false, false, Some(m)) => Valor::DataHora(m),
        (false, _, None) => Valor::Numero(serie),
    }
}

/// Texto ISO 8601 das células `t="d"`: `2024-03-15`, `2024-03-15T14:30:00(.123)(Z)`, `14:30:00`. O fuso é
/// ignorado (vale o horário escrito).
pub(crate) fn valor_de_iso(texto: &str) -> Option<Valor> {
    let t = texto.trim();
    let (parte_data, parte_hora) = match t.find(['T', 't', ' ']) {
        Some(i) => (&t[..i], Some(&t[i + 1..])),
        None if t.contains(':') => ("", Some(t)),
        None => (t, None),
    };
    let data = if parte_data.is_empty() { None } else { Some(data_iso(parte_data)?) };
    let hora = match parte_hora {
        Some(h) => Some(hora_iso(h)?),
        None => None,
    };
    match (data, hora) {
        (Some(data), Some(hora)) => Some(Valor::DataHora(Momento { data, hora })),
        (Some(data), None) => Some(Valor::Data(data)),
        (None, Some(hora)) => Some(Valor::Hora(hora)),
        (None, None) => None,
    }
}

fn data_iso(t: &str) -> Option<Data> {
    let mut p = t.splitn(3, '-');
    let ano: i32 = p.next()?.parse().ok()?;
    let mes: u8 = p.next()?.parse().ok()?;
    let dia: u8 = p.next()?.parse().ok()?;
    ((1..=12).contains(&mes) && dia >= 1 && dia <= dias_no_mes(ano, mes) && (0..=9999).contains(&ano)).then_some(Data {
        ano,
        mes,
        dia,
    })
}

fn hora_iso(t: &str) -> Option<Hora> {
    // tira o fuso: Z, +03:00, -03:00
    let sem_fuso = match t.find(['Z', 'z', '+']) {
        Some(i) => &t[..i],
        None => match t.rfind('-') {
            Some(i) if i > 0 => &t[..i],
            _ => t,
        },
    };
    let mut p = sem_fuso.split(':');
    let hora: u8 = p.next()?.parse().ok()?;
    let minuto: u8 = p.next().unwrap_or("0").parse().ok()?;
    let resto = p.next().unwrap_or("0");
    let (seg, frac) = match resto.split_once('.') {
        Some((s, f)) => (s, f),
        None => (resto, ""),
    };
    let segundo: u8 = seg.parse().ok()?;
    let milissegundo: u16 = if frac.is_empty() {
        0
    } else {
        let tres: String = frac.chars().chain("000".chars()).take(3).collect();
        tres.parse().ok()?
    };
    (hora < 24 && minuto < 60 && segundo < 60 && p.next().is_none()).then_some(Hora {
        hora,
        minuto,
        segundo,
        milissegundo,
    })
}

#[cfg(test)]
mod testes {
    use super::*;

    fn data(serie: f64, s: SistemaDatas) -> String {
        serial_para_momento(serie, s).map_or("fora".to_string(), |m| m.data.to_string())
    }

    #[test]
    fn sistema_1900_com_o_defeito_do_lotus() {
        let s = SistemaDatas::D1900;
        assert_eq!(data(0.0, s), "1900-01-00");
        assert_eq!(data(1.0, s), "1900-01-01");
        assert_eq!(data(31.0, s), "1900-01-31");
        assert_eq!(data(32.0, s), "1900-02-01");
        assert_eq!(data(59.0, s), "1900-02-28");
        assert_eq!(data(60.0, s), "1900-02-29");
        assert_eq!(data(61.0, s), "1900-03-01");
        assert_eq!(data(62.0, s), "1900-03-02");
        assert_eq!(data(25569.0, s), "1970-01-01");
        assert_eq!(data(36526.0, s), "2000-01-01");
        assert_eq!(data(36585.0, s), "2000-02-29");
        assert_eq!(data(44927.0, s), "2023-01-01");
        assert_eq!(data(45000.0, s), "2023-03-15");
        assert_eq!(data(2_958_465.0, s), "9999-12-31");
        assert_eq!(data(2_958_466.0, s), "fora");
    }

    #[test]
    fn sistema_1904() {
        let s = SistemaDatas::D1904;
        assert_eq!(data(0.0, s), "1904-01-01");
        assert_eq!(data(1.0, s), "1904-01-02");
        assert_eq!(data(1462.0, s), "1908-01-02");
        // a mesma data é 1462 dias a menos
        assert_eq!(data(45000.0 - 1462.0, s), "2023-03-15");
        assert_eq!(data(2_957_003.0, s), "9999-12-31");
        assert_eq!(data(2_957_004.0, s), "fora");
    }

    #[test]
    fn negativo_e_nao_numero_ficam_de_fora() {
        for x in [-1.0, -0.001, f64::NAN, f64::INFINITY, 1e12] {
            assert!(serial_para_momento(x, SistemaDatas::D1900).is_none(), "{x}");
            assert!(serial_para_hora(x).is_none(), "{x}");
        }
    }

    #[test]
    fn hora_arredondada_ao_milissegundo() {
        let h = |x: f64| serial_para_momento(x, SistemaDatas::D1900).unwrap().hora.to_string();
        assert_eq!(h(0.5), "12:00:00");
        assert_eq!(h(45000.75), "18:00:00");
        assert_eq!(h(0.25), "06:00:00");
        assert_eq!(h(0.999999), "23:59:59.914");
        assert_eq!(h(1.0 / 86400.0), "00:00:01");
        assert_eq!(h(0.604166666666667), "14:30:00");
        assert_eq!(h(0.000001), "00:00:00.086");
    }

    #[test]
    fn arredondamento_que_vira_o_dia() {
        let m = serial_para_momento(45_000.999_999_999_9, SistemaDatas::D1900).unwrap();
        assert_eq!(m.to_string(), "2023-03-16 00:00:00");
    }

    #[test]
    fn so_hora_ignora_o_dia() {
        assert_eq!(serial_para_hora(1.5).unwrap().to_string(), "12:00:00");
        assert_eq!(serial_para_hora(45000.25).unwrap().to_string(), "06:00:00");
    }

    #[test]
    fn valor_pela_categoria() {
        let s = SistemaDatas::D1900;
        assert_eq!(valor_de_serie(45000.5, true, false, s), Valor::Data(Data { ano: 2023, mes: 3, dia: 15 }));
        assert!(matches!(valor_de_serie(45000.5, false, false, s), Valor::DataHora(_)));
        assert!(matches!(valor_de_serie(45000.5, false, true, s), Valor::Hora(_)));
        assert_eq!(valor_de_serie(-5.0, true, false, s), Valor::Numero(-5.0));
        assert_eq!(valor_de_serie(-5.0, false, true, s), Valor::Numero(-5.0));
    }

    #[test]
    fn idas_e_voltas_do_calendario() {
        for dias in [-719_468, -25_569, -1, 0, 1, 11_016, 19_000, 47_482, 2_932_896] {
            assert_eq!(dias_de_data(data_de_dias(dias)), dias);
        }
        assert_eq!(data_de_dias(0), Data { ano: 1970, mes: 1, dia: 1 });
        assert_eq!(data_de_dias(11_016), Data { ano: 2000, mes: 2, dia: 29 });
    }

    #[test]
    fn dias_da_semana_como_o_excel() {
        let dow = |a, m, d| dia_da_semana(Data { ano: a, mes: m, dia: d });
        assert_eq!(dow(1970, 1, 1), 4); // quinta
        assert_eq!(dow(2023, 3, 15), 3); // quarta
        assert_eq!(dow(1900, 3, 1), 4); // quinta, de verdade
        assert_eq!(dow(1900, 1, 1), 0); // o Excel mostra domingo
        assert_eq!(dow(1900, 2, 29), 3); // quarta, no dia que não existiu
        assert_eq!(dow(2024, 2, 29), 4);
    }

    #[test]
    fn meses_e_bissextos() {
        assert_eq!(dias_no_mes(2024, 2), 29);
        assert_eq!(dias_no_mes(2023, 2), 28);
        assert_eq!(dias_no_mes(1900, 2), 28);
        assert_eq!(dias_no_mes(2000, 2), 29);
        assert_eq!(dias_no_mes(2023, 4), 30);
        assert_eq!(dias_no_mes(2023, 12), 31);
    }

    #[test]
    fn iso() {
        let d = |a, m, d| Data { ano: a, mes: m, dia: d };
        let h = |h, m, s, ms| Hora { hora: h, minuto: m, segundo: s, milissegundo: ms };
        assert_eq!(valor_de_iso("2024-03-15"), Some(Valor::Data(d(2024, 3, 15))));
        assert_eq!(
            valor_de_iso("2024-03-15T14:30:00"),
            Some(Valor::DataHora(Momento { data: d(2024, 3, 15), hora: h(14, 30, 0, 0) }))
        );
        assert_eq!(
            valor_de_iso("2024-03-15T14:30:05.5Z"),
            Some(Valor::DataHora(Momento { data: d(2024, 3, 15), hora: h(14, 30, 5, 500) }))
        );
        assert_eq!(
            valor_de_iso("2024-03-15T14:30-03:00"),
            Some(Valor::DataHora(Momento { data: d(2024, 3, 15), hora: h(14, 30, 0, 0) }))
        );
        assert_eq!(valor_de_iso("14:30:00"), Some(Valor::Hora(h(14, 30, 0, 0))));
        assert_eq!(valor_de_iso(" 2024-02-29 "), Some(Valor::Data(d(2024, 2, 29))));
        for ruim in ["2023-02-29", "2024-13-01", "2024-00-10", "ontem", "", "2024-03-15T25:00:00", "2024-03"] {
            assert_eq!(valor_de_iso(ruim), None, "{ruim}");
        }
    }
}

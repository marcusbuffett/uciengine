use log::{error, warn};

use envor::envor::env_true;

use serde::{Deserialize, Serialize};

use thiserror::Error;

/// InfoParseError captures possible info parsing errors
#[derive(Error, Debug)]
pub enum InfoParseError {
    #[error("could not parse info number for state '{0:?}' from '{1}'")]
    ParseNumberError(ParsingState, String),
    #[error("invalid info key '{0}'")]
    InvalidKeyError(String),
    #[error("invalid score specifier '{0}'")]
    InvalidScoreSpecifier(String),
}

/// log info parse error and return it as a result
pub fn info_parse_error(err: InfoParseError) -> Result<(), InfoParseError> {
    error!("{:?}", err);

    Err(err)
}

/// log parse number error and return it as a result
pub fn parse_number_error<T: AsRef<str>>(ps: ParsingState, value: T) -> Result<(), InfoParseError> {
    let value = value.as_ref().to_string();

    info_parse_error(InfoParseError::ParseNumberError(ps, value))
}

/// generate string buffer with given name and size
macro_rules! gen_str_buff {
	($(#[$attr:meta] => $type:ident, $size:expr),*) => { $(
	    #[$attr]
	    #[derive(Clone, Copy)]
		pub struct $type {
			pub len: usize,
			pub buff: [u8; $size],
		}

		#[$attr]
		#[doc = "implementation"]
		impl $type {
			#[doc = "create new"]
			#[$attr]
			pub fn new() -> Self {
				Self {
					len: 0,
					buff: [0; $size]
				}
			}

			#[doc = "convert"]
			#[$attr]
			#[doc = "to option ( None if empty, Some(contents) otherwise )"]
			pub fn to_opt(self) -> Option<String> {
				if self.len == 0 {
					return None;
				}

				Some(String::from(self))
			}

			#[doc = "set"]
			#[$attr]
			#[doc = "( value will be trimmed to buffer size )"]
			pub fn set<T: AsRef<str>>(&mut self, value: T) -> Self {
				let bytes = value.as_ref().as_bytes();

				let mut len = bytes.len();

				if len > $size{
					len = $size;
				}

				self.len = len;

				self.buff[0..len].copy_from_slice(&bytes[0..len]);

				*self
			}

			#[doc = "reset"]
			#[$attr]
			#[doc = "to empty buffer"]
			pub fn reset(&mut self) -> Self {
				self.len = 0;

				*self
			}

			pub fn set_trim<T: AsRef<str>>(&mut self, value: T, trim: char) -> Self {
				let value_ref = value.as_ref();
				let value_string = value_ref.to_string();
				let bytes = value_ref.as_bytes();

				let mut total_len = value_string.len();

			    value_ref.to_string().chars().rev().take_while(|c| {
			        total_len -= 1;
			        ( *c != trim ) || ( total_len > $size )
			    }).collect::<String>().len();

			    self.len = total_len;

			    self.buff[0..total_len].copy_from_slice(&bytes[0..total_len]);

				*self
			}
		}

		#[doc = "implement From<&str> for"]
		#[$attr]
		impl std::convert::From<&str> for $type {
			fn from(value: &str) -> Self {
				let bytes = value.as_bytes();

				let mut len = bytes.len();

				if len > $size{
					len = $size;
				}

				let mut buff = $type::new();

                buff.len = len;
				buff.buff[0..len].copy_from_slice(&bytes[0..len]);

				buff
			}
		}

		#[doc = "implement From<String> for"]
		#[$attr]
		impl std::convert::From<String> for $type {
			fn from(value: String) -> Self {
				Self::from(value.as_str())
			}
		}

		#[doc = "implement From<Option<String>> for"]
		#[$attr]
		impl std::convert::From<Option<String>> for $type {
			fn from(value: Option<String>) -> Self {
				Self::from(value.unwrap_or(String::new()).as_str())
			}
		}

		#[doc = "implement From<"]
		#[$attr]
		#[doc = "> for String"]
		impl std::convert::From<$type> for String {
			fn from(buff: $type) -> String {
				std::str::from_utf8(&buff.buff[0..buff.len]).unwrap().to_string()
			}
		}

		#[doc = "implement Display for"]
		#[$attr]
		impl std::fmt::Display for $type {
			fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		        write!(f, "{}", String::from(*self))
		    }
		}

		#[doc = "implement Debug for"]
		#[$attr]
		impl std::fmt::Debug for $type {
			fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		        write!(f, "[{}[{}]: '{}']", stringify!($type), self.len, String::from(*self))
		    }
		}
	)* }
}

/// maximum length of uci move
const UCI_MAX_LENGTH: usize = 5;
/// typical length of uci move
const UCI_TYPICAL_LENGTH: usize = 4;
/// maximum number of pv moves to store
#[cfg(not(test))]
const MAX_PV_MOVES: usize = 10;
#[cfg(test)]
const MAX_PV_MOVES: usize = 2;
/// pv buffer size
const PV_BUFF_SIZE: usize = MAX_PV_MOVES * (UCI_TYPICAL_LENGTH + 1);

gen_str_buff!(
/// UciBuff
=> UciBuff, UCI_MAX_LENGTH,
/// PvBuff
=> PvBuff, PV_BUFF_SIZE
);

/// score
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Score {
    /// centipawn
    Cp(i32),
    /// mate
    Mate(i32),
}

/// score type
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ScoreType {
    /// exact
    Exact,
    /// lowerbound
    Lowerbound,
    /// upperbound
    Upperbound,
}

// http://wbec-ridderkerk.nl/html/UCIProtocol.html
//
// * info
// 	the engine wants to send infos to the GUI. This should be done whenever one of the info has changed.
// 	The engine can send only selected infos and multiple infos can be send with one info command,
// 	e.g. "info currmove e2e4 currmovenumber 1" or
// 	     "info depth 12 nodes 123456 nps 100000".
// 	Also all infos belonging to the pv should be sent together
// 	e.g. "info depth 2 score cp 214 time 1242 nodes 2124 nps 34928 pv e2e4 e7e5 g1f3"
// 	I suggest to start sending "currmove", "currmovenumber", "currline" and "refutation" only after one second
// 	to avoid too much traffic.
// 	Additional info:
// 	* depth
// 		search depth in plies
// 	* seldepth
// 		selective search depth in plies,
// 		if the engine sends seldepth there must also a "depth" be present in the same string.
// 	* time
// 		the time searched in ms, this should be sent together with the pv.
// 	* nodes
// 		x nodes searched, the engine should send this info regularly
// 	* pv  ...
// 		the best line found
// 	* multipv
// 		this for the multi pv mode.
// 		for the best move/pv add "multipv 1" in the string when you send the pv.
// 		in k-best mode always send all k variants in k strings together.
// 	* score
// 		* cp
// 			the score from the engine's point of view in centipawns.
// 		* mate
// 			mate in y moves, not plies.
// 			If the engine is getting mated use negativ values for y.
// 		* lowerbound
// 	      the score is just a lower bound.
// 		* upperbound
// 		   the score is just an upper bound.
// 	* currmove
// 		currently searching this move
// 	* currmovenumber
// 		currently searching move number x, for the first move x should be 1 not 0.
// 	* hashfull
// 		the hash is x permill full, the engine should send this info regularly
// 	* nps
// 		x nodes per second searched, the engine should send this info regularly
// 	* tbhits
// 		x positions where found in the endgame table bases
// 	* cpuload
// 		the cpu usage of the engine is x permill.
// 	* string
// 		any string str which will be displayed be the engine,
// 		if there is a string command the rest of the line will be interpreted as .
// 	* refutation   ...
// 	   move  is refuted by the line  ... , i can be any number >= 1.
// 	   Example: after move d1h5 is searched, the engine can send
// 	   "info refutation d1h5 g6h5"
// 	   if g6h5 is the best answer after d1h5 or if g6h5 refutes the move d1h5.
// 	   if there is norefutation for d1h5 found, the engine should just send
// 	   "info refutation d1h5"
// 		The engine should only send this if the option "UCI_ShowRefutations" is set to true.
// 	* currline   ...
// 	   this is the current line the engine is calculating.  is the number of the cpu if
// 	   the engine is running on more than one cpu.  = 1,2,3....
// 	   if the engine is just using one cpu,  can be omitted.
// 	   If  is greater than 1, always send all k lines in k strings together.
// 		The engine should only send this if the option "UCI_ShowCurrLine" is set to true.

/// analysis info
#[derive(Debug, Clone, Copy)]
pub struct AnalysisInfo {
    /// false for ongoing analysis, true when analysis stopped on bestmove received
    pub done: bool,
    /// best move
    bestmove: UciBuff,
    /// ponder
    ponder: UciBuff,
    /// pv
    pv: PvBuff,
    /// depth
    pub depth: usize,
    /// seldepth
    pub seldepth: usize,
    /// time
    pub time: usize,
    /// nodes
    pub nodes: u64,
    /// multipv
    pub multipv: usize,
    /// score ( centipawns or mate )
    pub score: Score,
    /// current move
    pub currmove: UciBuff,
    /// current move number
    pub currmovenumber: usize,
    /// hashfull
    pub hashfull: usize,
    /// nodes per second
    pub nps: u64,
    /// tbhits
    pub tbhits: u64,
    /// cpuload
    pub cpuload: usize,
    /// score type
    pub scoretype: ScoreType,
    pub wdl: WDL,
}

/// analysis info serde
#[derive(Debug, Serialize, Deserialize)]
pub struct AnalysisInfoSerde {
    /// disposition
    pub disposition: String,
    /// false for ongoing analysis, true when analysis stopped on bestmove received
    pub done: bool,
    /// best move
    pub bestmove: Option<String>,
    /// ponder
    pub ponder: Option<String>,
    /// pv
    pub pv: Option<String>,
    /// depth
    pub depth: usize,
    /// seldepth
    pub seldepth: usize,
    /// time
    pub time: usize,
    /// nodes
    pub nodes: u64,
    /// multipv
    pub multipv: usize,
    /// score ( centipawns or mate )
    pub score: Score,
    pub wdl: WDL,
    /// current move
    pub currmove: Option<String>,
    /// current move number
    pub currmovenumber: usize,
    /// hashfull
    pub hashfull: usize,
    /// nodes per second
    pub nps: u64,
    /// tbhits
    pub tbhits: u64,
    /// cpuload
    pub cpuload: usize,
    /// score type
    pub scoretype: ScoreType,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct WDL {
    pub win: u64,
    pub draw: u64,
    pub loss: u64,
}

/// parsing state
#[derive(Debug)]
#[allow(dead_code)]
// TODO: make this pub(crate)
pub enum ParsingState {
    Info,
    Key,
    Unknown,
    Depth,
    Seldepth,
    Time,
    Nodes,
    Multipv,
    Score,
    WdlW,
    WdlD,
    WdlL,
    ScoreCp,
    ScoreMate,
    Currmove,
    Currmovenumber,
    Hashfull,
    Nps,
    Tbhits,
    Cpuload,
    PvBestmove,
    PvPonder,
    PvRest,
}

/// analysis info implementation
impl AnalysisInfo {
    /// create new analysis info
    pub fn new() -> Self {
        Self {
            done: false,
            bestmove: UciBuff::new(),
            ponder: UciBuff::new(),
            pv: PvBuff::new(),
            depth: 0,
            seldepth: 0,
            time: 0,
            nodes: 0,
            multipv: 0,
            score: Score::Cp(0),
            currmove: UciBuff::new(),
            currmovenumber: 0,
            hashfull: 0,
            nps: 0,
            tbhits: 0,
            cpuload: 0,
            scoretype: ScoreType::Exact,
            wdl: WDL {
                win: 0,
                draw: 0,
                loss: 0,
            },
        }
    }

    /// to serde
    pub fn to_serde(self) -> AnalysisInfoSerde {
        AnalysisInfoSerde {
            disposition: "AnalysisInfo".to_string(),
            done: self.done,
            bestmove: self.bestmove(),
            ponder: self.ponder(),
            pv: self.pv(),
            depth: self.depth,
            seldepth: self.seldepth,
            time: self.time,
            nodes: self.nodes,
            multipv: self.multipv,
            score: self.score,
            currmove: self.currmove(),
            currmovenumber: self.currmovenumber,
            hashfull: self.hashfull,
            nps: self.nps,
            tbhits: self.tbhits,
            cpuload: self.cpuload,
            scoretype: self.scoretype,
            wdl: self.wdl,
        }
    }

    /// from serde
    pub fn from_serde(ais: AnalysisInfoSerde) -> Self {
        Self {
            done: ais.done,
            bestmove: UciBuff::from(ais.bestmove),
            ponder: UciBuff::from(ais.ponder),
            pv: PvBuff::from(ais.pv),
            depth: ais.depth,
            seldepth: ais.seldepth,
            time: ais.time,
            nodes: ais.nodes,
            multipv: ais.multipv,
            score: ais.score,
            currmove: UciBuff::from(ais.currmove),
            currmovenumber: ais.currmovenumber,
            hashfull: ais.hashfull,
            nps: ais.nps,
            tbhits: ais.tbhits,
            cpuload: ais.cpuload,
            scoretype: ais.scoretype,
            wdl: ais.wdl,
        }
    }

    /// from json
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        match serde_json::from_str::<AnalysisInfoSerde>(json) {
            Ok(ais) => Ok(AnalysisInfo::from_serde(ais)),
            Err(err) => Err(err),
        }
    }

    /// to json
    pub fn to_json(self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&self.to_serde())
    }

    // get bestmove
    pub fn bestmove(self) -> Option<String> {
        self.bestmove.to_opt()
    }

    // get ponder
    pub fn ponder(self) -> Option<String> {
        self.ponder.to_opt()
    }

    // get pv
    pub fn pv(self) -> Option<String> {
        self.pv.to_opt()
    }

    // get current move
    pub fn currmove(self) -> Option<String> {
        self.currmove.to_opt()
    }

    /// parse info string
    pub fn parse<T: std::convert::AsRef<str>>(&mut self, info: T) -> Result<(), InfoParseError> {
        let info = info.as_ref();
        let mut ps = ParsingState::Info;
        let mut pv_buff = String::new();
        let mut pv_on = false;

        let allow_unknown_key = env_true("ALLOW_UNKNOWN_INFO_KEY");

        for token in info.split(" ") {
            match ps {
                ParsingState::Info => {
                    match token {
                        "info" => ps = ParsingState::Key,
                        _ => {
                            // not an info
                            return Ok(());
                        }
                    }
                }
                ParsingState::Key => {
                    if (token == "string") || (token == "refutation") || (token == "currline") {
                        // string, refutation and currline are not supported
                        return Ok(());
                    }

                    ps = match token {
                        "lowerbound" => {
                            self.scoretype = ScoreType::Lowerbound;

                            ParsingState::Key
                        }
                        "upperbound" => {
                            self.scoretype = ScoreType::Upperbound;

                            ParsingState::Key
                        }
                        "depth" => ParsingState::Depth,
                        "seldepth" => ParsingState::Seldepth,
                        "time" => ParsingState::Time,
                        "nodes" => ParsingState::Nodes,
                        "multipv" => ParsingState::Multipv,
                        "score" => ParsingState::Score,
                        "wdl" => ParsingState::WdlW,
                        "currmove" => ParsingState::Currmove,
                        "currmovenumber" => ParsingState::Currmovenumber,
                        "hashfull" => ParsingState::Hashfull,
                        "nps" => ParsingState::Nps,
                        "tbhits" => ParsingState::Tbhits,
                        "cpuload" => ParsingState::Cpuload,
                        "pv" => ParsingState::PvBestmove,
                        _ => {
                            if allow_unknown_key {
                                ParsingState::Unknown
                            } else {
                                return Err(InfoParseError::InvalidKeyError(token.to_string()));
                            }
                        }
                    };

                    if let ParsingState::Score = ps {
                        self.scoretype = ScoreType::Exact;
                    }
                }
                ParsingState::Score => match token {
                    "cp" => ps = ParsingState::ScoreCp,
                    "mate" => ps = ParsingState::ScoreMate,
                    "upperbound" => self.scoretype = ScoreType::Upperbound,
                    "lowerbound" => self.scoretype = ScoreType::Lowerbound,
                    _ => {
                        // not a valid score specifier
                        return info_parse_error(InfoParseError::InvalidScoreSpecifier(
                            token.to_string(),
                        ));
                    }
                },
                ParsingState::Unknown => {
                    // ignore this token and hope for the best ( namely that it had a single token arg )
                    warn!("unknown info key {}", token);

                    ps = ParsingState::Key
                }
                _ => {
                    let mut keep_state = false;

                    match ps {
                        ParsingState::Depth => match token.parse::<usize>() {
                            Ok(depth) => self.depth = depth,
                            _ => return parse_number_error(ps, token),
                        },
                        ParsingState::Seldepth => match token.parse::<usize>() {
                            Ok(seldepth) => self.seldepth = seldepth,
                            _ => return parse_number_error(ps, token),
                        },
                        ParsingState::Time => match token.parse::<usize>() {
                            Ok(time) => self.time = time,
                            _ => return parse_number_error(ps, token),
                        },
                        ParsingState::WdlW => {
                            match token.parse::<u64>() {
                                Ok(x) => self.wdl.win = x,
                                _ => return parse_number_error(ps, token),
                            }
                            ps = ParsingState::WdlD;
                            keep_state = true;
                        }
                        ParsingState::WdlD => {
                            match token.parse::<u64>() {
                                Ok(x) => self.wdl.draw = x,
                                _ => return parse_number_error(ps, token),
                            }
                            ps = ParsingState::WdlL;
                            keep_state = true;
                        }
                        ParsingState::WdlL => match token.parse::<u64>() {
                            Ok(x) => self.wdl.loss = x,
                            _ => return parse_number_error(ps, token),
                        },
                        ParsingState::Nodes => match token.parse::<u64>() {
                            Ok(nodes) => self.nodes = nodes,
                            _ => return parse_number_error(ps, token),
                        },
                        ParsingState::Multipv => match token.parse::<usize>() {
                            Ok(multipv) => self.multipv = multipv,
                            _ => return parse_number_error(ps, token),
                        },
                        ParsingState::ScoreCp => match token {
                            "upperbound" => {
                                self.scoretype = ScoreType::Upperbound;

                                keep_state = true
                            }
                            "lowerbound" => {
                                self.scoretype = ScoreType::Lowerbound;

                                keep_state = true
                            }
                            _ => match token.parse::<i32>() {
                                Ok(score_cp) => self.score = Score::Cp(score_cp),
                                _ => return parse_number_error(ps, token),
                            },
                        },
                        ParsingState::ScoreMate => match token {
                            "upperbound" => {
                                self.scoretype = ScoreType::Upperbound;

                                keep_state = true
                            }
                            "lowerbound" => {
                                self.scoretype = ScoreType::Lowerbound;

                                keep_state = true
                            }
                            _ => match token.parse::<i32>() {
                                Ok(score_mate) => self.score = Score::Mate(score_mate),
                                _ => return parse_number_error(ps, token),
                            },
                        },
                        ParsingState::Currmove => {
                            self.currmove.set(token);

                            ()
                        }
                        ParsingState::Currmovenumber => match token.parse::<usize>() {
                            Ok(currmovenumber) => self.currmovenumber = currmovenumber,
                            _ => return parse_number_error(ps, token),
                        },
                        ParsingState::Hashfull => match token.parse::<usize>() {
                            Ok(hashfull) => self.hashfull = hashfull,
                            _ => return parse_number_error(ps, token),
                        },
                        ParsingState::Nps => match token.parse::<u64>() {
                            Ok(nps) => self.nps = nps,
                            _ => return parse_number_error(ps, token),
                        },
                        ParsingState::Tbhits => match token.parse::<u64>() {
                            Ok(tbhits) => self.tbhits = tbhits,
                            _ => return parse_number_error(ps, token),
                        },
                        ParsingState::Cpuload => match token.parse::<usize>() {
                            Ok(cpuload) => self.cpuload = cpuload,
                            _ => return parse_number_error(ps, token),
                        },
                        ParsingState::PvBestmove => {
                            pv_buff = pv_buff + token;

                            self.bestmove = UciBuff::from(token);

                            self.ponder.reset();

                            pv_on = true;

                            ps = ParsingState::PvPonder
                        }
                        ParsingState::PvPonder => {
                            pv_buff = pv_buff + " " + token;

                            self.ponder = UciBuff::from(token);

                            ps = ParsingState::PvRest
                        }
                        ParsingState::PvRest => pv_buff = pv_buff + " " + token,
                        _ => {
                            // should not happen
                        }
                    }

                    // anything from key pv onwards should be added to pv
                    // otherwise switch back to parsing key
                    if (!pv_on) && (!keep_state) {
                        ps = ParsingState::Key;
                    }
                }
            }
        }

        self.pv.set_trim(pv_buff, ' ');

        Ok(())
    }
}

#[test]
fn set_trim() {
    let mut x = PvBuff::new().set("e2e4");

    assert_eq!(x.len, 4);

    assert_eq!(String::from(x), "e2e4".to_string());

    x.set_trim("e2e4 e7e5 g1f3 b8c6", ' ');

    assert_eq!(x.len, 9);

    assert_eq!(String::from(x), "e2e4 e7e5".to_string());
}

#[test]
fn parse_error() {
    let mut ai = AnalysisInfo::new();

    let _ = ai.parse(
        "info depth 3 score mate 5 nodes 3000000000 time 3000 nps 1000000 pv e2e4 e7e5 g1f3",
    );

    assert_eq!(ai.depth, 3);
    assert_eq!(format!("{:?}", ai.score), format!("{:?}", Score::Mate(5)));
    assert_eq!(format!("{:?}", ai.ponder()), format!("{:?}", Some("e7e5")));
}

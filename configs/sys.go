package configs

// type and version
const Version = "CESS-Bucket_V0.3.0"

// cess chain module
const (
	ChainModule_Sminer      = "Sminer"
	ChainModule_SegmentBook = "SegmentBook"
)

// cess chain module method
const (
	ChainModule_Sminer_MinerItems          = "MinerItems"
	ChainModule_Sminer_MinerDetails        = "MinerDetails"
	ChainModule_Sminer_SegInfo             = "SegInfo"
	ChainModule_SegmentBook_ParamSetA      = "ParamSetA"
	ChainModule_SegmentBook_ParamSetB      = "ParamSetB"
	ChainModule_SegmentBook_ParamSetD      = "ParamSetD"
	ChainModule_SegmentBook_ConProofInfoA  = "ConProofInfoA"
	ChainModule_SegmentBook_ConProofInfoC  = "ConProofInfoC"
	ChainModule_SegmentBook_MinerHoldSlice = "MinerHoldSlice"
)

// cess chain Transaction name
const (
	ChainTx_Sminer_Register              = "Sminer.regnstk"
	ChainTx_SegmentBook_IntentSubmit     = "SegmentBook.intent_submit"
	ChainTx_SegmentBook_IntentSubmitPost = "SegmentBook.intent_submit_po_st"
	ChainTx_FileBank_Update              = "FileBank.update"
	ChainTx_SegmentBook_SubmitToVpa      = "SegmentBook.submit_to_vpa"
	ChainTx_SegmentBook_SubmitToVpb      = "SegmentBook.submit_to_vpb"
	ChainTx_SegmentBook_SubmitToVpc      = "SegmentBook.submit_to_vpc"
	ChainTx_SegmentBook_SubmitToVpd      = "SegmentBook.submit_to_vpd"
	ChainTx_Sminer_ExitMining            = "Sminer.exit_miner"
	ChainTx_Sminer_Withdraw              = "Sminer.withdraw"
	ChainTx_Sminer_Increase              = "Sminer.increase_collateral"
)

const (
	RpcService_Scheduler          = "wservice"
	RpcMethod_Scheduler_Writefile = "writefile"
	RpcMethod_Scheduler_Readfile  = "readfile"
)

const (
	SegMentType_8M     uint8 = 1
	SegMentType_8M_S         = "1"
	SegMentType_512M   uint8 = 2
	SegMentType_512M_S       = "2"
	FileSealProof            = 1
	FilePostProof            = 6
)

const (
	Vpb_SubmintPeriod  = 72
	Vpd_SubmintPeriod  = 72
	TimeToWaitEvents_S = 20
)

const (
	MinimumSpace                 = 1099511627776 // 1TB
	Space_1GB                    = 1073741824    // 1GB
	TokenAccuracy                = "000000000000"
	DefaultConfigurationFileName = "config_template.toml"
	LengthOfFileShardMeta        = 100
)

// Miner data updated at runtime
var (
	MinerId_S        string = ""
	MinerId_I        uint64 = 0
	MinerDataPath    string = "cessminer_c"
	MinerUseSpace    uint64 = 0
	MinerServiceAddr string = ""
	MinerServicePort int    = 0
)

var (
	LogfilePathPrefix = "/log"
	SpaceDir          = "space"
	ServiceDir        = "service"
	Cache             = "cache"
	TmpltFileFolder   = "temp"
	TmpltFileName     = "template"
)

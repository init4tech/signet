// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity 0.8.26;

// test contract
import {RollupPassage} from "zenith/src/passage/RollupPassage.sol";
// utils
import {AllocsTest} from "./AllocsTest.t.sol";
import {TestERC20} from "zenith/test/Helpers.t.sol";
import {RollupPassagePermit2Test} from "zenith/test/Permit2Passage.t.sol";
import {ISignatureTransfer} from "permit2/src/interfaces/ISignatureTransfer.sol";

contract AllocsPermit2RollupPassageTest is AllocsTest, RollupPassagePermit2Test {
    function setUp() public override {
        vm.loadAllocs(ALLOCS_FILE_PATH);
        vm.chainId(CHAIN_ID);

        target = RollupPassage(payable(ROLLUP_PASSAGE));
        permit2Contract = PERMIT2;
        token = tokens[0];

        _mintTokens(owner, amount * 1000);
        _approveTokens(owner, amount * 1000, permit2Contract);

        _setupSinglePermit(token, amount);

        // construct Exit witness
        witness = target.exitWitness(recipient);

        // sign permit + witness
        permit2.signature = signPermit(ownerKey, address(target), permit2.permit, witness);

        // construct transfer details
        transferDetails = ISignatureTransfer.SignatureTransferDetails({to: address(target), requestedAmount: amount});
    }

    // run tests written for local chain env, pointed to contracts deployed in genesis file}
}

// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity 0.8.26;

// test contract
import {RollupPassage} from "zenith/src/passage/RollupPassage.sol";
// utils
import {AllocsTest} from "./AllocsTest.t.sol";
import {RollupPassageTest} from "zenith/test/Passage.t.sol";

contract AllocsRollupPassageTest is AllocsTest, RollupPassageTest {
    function setUp() public override {
        vm.loadAllocs(ALLOCS_FILE_PATH);

        target = RollupPassage(payable(ROLLUP_PASSAGE));

        _mintTokens(address(this), amount * 1000);
        _approveTokens(address(this), amount * 1000, address(target));

        token = tokens[0];
    }

    // run tests written for local chain env, pointed to contracts deployed in genesis file}
}

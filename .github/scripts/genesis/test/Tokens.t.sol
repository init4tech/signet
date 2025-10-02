// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity 0.8.26;

// utils
import {AllocsTest} from "./AllocsTest.t.sol";
import {ISimpleToken} from "./interfaces/ISimpleToken.sol";
import {Ownable} from "zenith/lib/openzeppelin-contracts/contracts/access/Ownable.sol";

// Test that the WBTC token contract is configured correctly & functions as expected
contract AllocsWBTCTest is AllocsTest {
    ISimpleToken target;

    event Transfer(address indexed from, address indexed to, uint256 value);
    event Mint(address indexed minter, address indexed to, uint256 amount);

    function setUp() public virtual {
        vm.loadAllocs(ALLOCS_FILE_PATH);
        target = ISimpleToken(payable(WBTC));
    }

    // test that token metadata is setup correctly
    function test_metadata() external virtual {
        assertEq(target.name(), "Wrapped BTC", "name");
        assertEq(target.symbol(), "WBTC", "symbol");
        assertEq(target.decimals(), 8, "decimals");
    }

    // test that the MINTER key is the Owner
    function test_owner() external {
        assertEq(Ownable(address(target)).owner(), MINTER, "owner");
    }

    // test that the Minter key can mint tokens
    function test_mint() external {
        vm.startPrank(MINTER);

        // emits Transfer
        vm.expectEmit();
        emit Transfer(address(0), address(this), 100);
        target.mint(address(this), 100);

        assertEq(target.balanceOf(address(this)), 100);
    }
}

// Test that the USDT token contract is configured correctly & functions as expected
contract AllocsWETHTest is AllocsWBTCTest {
    function setUp() public override {
        vm.loadAllocs(ALLOCS_FILE_PATH);
        target = ISimpleToken(payable(WETH));
    }

    // test that token metadata is setup correctly
    function test_metadata() external override {
        assertEq(target.name(), "Wrapped Ether", "name");
        assertEq(target.symbol(), "WETH", "symbol");
        assertEq(target.decimals(), 18, "decimals");
    }
}
